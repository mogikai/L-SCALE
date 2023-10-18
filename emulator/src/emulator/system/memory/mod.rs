use std::cmp::max;
use std::collections::btree_set::Iter;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::once;
use std::marker::PhantomData;
use std::ops::{Add, BitAnd, Not, Range};

use bitflags::Flags;
use byteorder::ByteOrder;
use generational_arena::{Arena, Index};

pub use accessor::MemoryAccessor;
use layout::Layout;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU, MREMAP_FLAGS};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::{FileDescriptorData, FD};
// TODO: Make TaskId generic
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::fd_map::{FileDescriptor, FileDescriptorMap, InternalFD};
use crate::emulator::system::memory::helpers::is4kaligned;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::rw_interface::DirectMemoryWrite;
use crate::emulator::system::memory::tlb::mapping::Mapping;
pub use crate::emulator::system::memory::tlb::mapping::MappingType;
pub use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;
use crate::emulator::system::memory::tlb::Tlb;

mod accessor;
pub mod helpers;
pub mod layout;
pub mod pregion;
mod restoreable;
pub mod rw_interface;
mod tlb;

macro_rules! layout {
    ($self:ident, $task_id:ident) => {
        $self
            .layouts
            .get(&$task_id)
            .expect("Error in emulator! The layout associated with the task_id could not be found!")
    };
}

macro_rules! layout_mut {
    ($self:ident, $task_id:ident) => {
        $self
            .layouts
            .get_mut(&$task_id)
            .expect("Error in emulator! The layout associated with the task_id could not be found!")
    };
}

macro_rules! tlb {
    ($self:ident, $task_id:ident) => {
        $self
            .tlbs
            .get(layout!($self, $task_id).tlb)
            .expect("Error in emulator! The tlb associated with the layout was not found!")
    };
}

macro_rules! l_tlb {
    ($self:ident, $layout:ident) => {
        $self
            .tlbs
            .get($layout.tlb)
            .expect("Error in emulator! The layout associated with the task_id could not be found!")
    };
}

macro_rules! tlb_mut {
    ($self:ident, $task_id:ident) => {
        $self
            .tlbs
            .get_mut(layout!($self, $task_id).tlb)
            .expect("Error in emulator! The tlb associated with the layout was not found!")
    };
}

macro_rules! l_tlb_mut {
    ($self:ident, $layout:ident) => {
        $self
            .tlbs
            .get_mut($layout.tlb)
            .expect("Error in emulator! The layout associated with the task_id could not be found!")
    };
}

/// Contains the memory layout of the current process task. Each process has its own copy of the memory layout.
/// Multiple virtual addresses may be mapped to the same physical address.
/// The Memory layout may be shared between tasks (i.e. threads) (Mostly only the VTLB)
#[derive(Debug)]
pub struct Memory<Endianness: ByteOrder> {
    /// The virtual address layout of each process/thread (stack, heap, mappings, etc.)
    /// and the VTLB containing the mappings from emulated virtual addresses to emulated physical addresses.
    layouts: HashMap<TaskId, Layout>,
    tlbs: Arena<Tlb>,
    unused_physical_sets: Arena<BTreeSet<PhysicalRegion>>,

    /// The end of the allocated Emulated Physical address space.
    /// New processes are placed here and the end is then extended to after the new process.
    physical_end: EPAddr,

    /// The size of emulated physical memory that is allocated per process.
    process_mem_size: usize,

    phantom_data: PhantomData<Endianness>,

    snapshots: Vec<MemorySnapshot>,
}

/// A memory snapshot. Currently we just create a deep copy of the entire layouts map.
#[derive(Debug)]
struct MemorySnapshot {
    layouts: HashMap<TaskId, Layout>,
    tlbs: Arena<Tlb>,
    unused_physical_sets: Arena<BTreeSet<PhysicalRegion>>,
    physical_end: EPAddr,
}

impl<Endianness: ByteOrder> Default for Memory<Endianness> {
    fn default() -> Self {
        Memory::new()
    }
}

impl<Endianness: ByteOrder> Memory<Endianness> {
    pub fn new() -> Self {
        Self {
            layouts: Default::default(),
            tlbs: Default::default(),
            unused_physical_sets: Default::default(),
            physical_end: helpers::align4k_down(EPAddr::from(0x8_0000_0000)),
            process_mem_size: helpers::align4k_down(1024u64 * 1024u64 * 1024u64) as usize, // 1 GiB
            phantom_data: Default::default(),
            snapshots: vec![],
        }
    }

    pub fn accessor<'a, MmuT: MMU>(
        &'a mut self,
        mmu: &'a mut MmuT,
        task_id: TaskId,
    ) -> MemoryAccessor<'a, Endianness, MmuT> {
        MemoryAccessor {
            memory: self,
            mmu,
            task_id,
        }
    }

    /// Creates a new process with the supplied TaskId. The mmu is needed to map in the physical
    /// memory that will be used by the process.
    pub fn new_process(
        &mut self,
        mmu: &mut impl MMU,
        task_id: TaskId,
    ) -> Result<(), MemoryError> {
        if self.layouts.contains_key(&task_id) {
            Err(MemoryError::ProcessAlreadyExists)?
        }

        let unused_physical = self
            .unused_physical_sets
            .insert(BTreeSet::from_iter(once(PhysicalRegion::new(
                self.physical_end,
                self.physical_end + self.process_mem_size,
            ))));
        let tlb = self.tlbs.insert(Tlb::new());
        let mut layout = Layout::new(
            0x0000_7fff_ffff_0000.into(),
            10 * 1024 * 1024, // 10 MiB
            0x0000_5000_0000_0000.into(),
            0x0000_3000_0000_0000.into(),
            unused_physical,
            tlb,
        );

        // stack
        let stack_physical_region = reserve_last_free_physical_region(
            self.unused_physical_sets
                .get_mut(unused_physical)
                .expect("We just inserted this index"),
            layout.max_stack,
        )?;

        l_tlb_mut!(self, layout).try_map(
            layout.stack_end - layout.max_stack,
            stack_physical_region.start,
            layout.max_stack,
            Permission::READ | Permission::WRITE,
            MappingType::PrivateAnonymous,
            Some("Stack".to_string()),
        )?;

        mmu.mem_map(self.physical_end, self.process_mem_size, Permission::ALL)?;
        self.physical_end += self.process_mem_size;

        let max_stack = layout.max_stack;
        let stack_end = layout.stack_end;
        let old_layout = self.layouts.insert(task_id, layout);
        assert!(old_layout.is_none());

        self.accessor(mmu, task_id)
            .write_direct_down(stack_end, vec![0u8; max_stack].as_slice())
            .unwrap();
        Ok(())
    }

    pub fn iterator_mappings<F, R, E>(
        &self,
        task_id: &TaskId,
        mut callback: F,
    ) -> Result<R, MemoryError>
    where
        F: FnMut(Iter<Mapping>) -> Result<R, E>,
        MemoryError: From<E>,
    {
        Ok(callback(tlb!(self, task_id).mapping_iter())?)
    }

    pub fn translate_vaddr(
        &self,
        task_id: TaskId,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        tlb!(self, task_id).translate_virtual_address(address)
    }

    pub fn translate_vaddr_range(
        &self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<TLBEntry>, MemoryError> {
        tlb!(self, task_id).translate_virtual_addresses(address, size)
    }

    pub(crate) fn stack_base(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        Ok(layout!(self, task_id).stack_end)
    }

    pub(crate) fn stack_size(
        &self,
        task_id: TaskId,
    ) -> Result<usize, MemoryError> {
        Ok(layout!(self, task_id).max_stack)
    }

    pub(crate) fn heap_end(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        Ok(layout!(self, task_id).heap_end)
    }

    pub(crate) fn realloc_heap(
        &mut self,
        task_id: TaskId,
        new_heap_end: EVAddr,
    ) -> Result<(), MemoryError> {
        let layout = layout_mut!(self, task_id);

        if new_heap_end < layout.heap_base {
            Err(MemoryError::Unknown)?
        }

        let size: usize = helpers::align4k_up(new_heap_end - layout.heap_end);
        if size != 0 {
            self.increase_heap(task_id, size)?;
        }

        Ok(())
    }
    fn increase_heap(
        &mut self,
        task_id: TaskId,
        size: usize,
    ) -> Result<(), MemoryError> {
        if size == 0 {
            Err(MemoryError::ZeroSize)?
        }
        let layout = layout_mut!(self, task_id);
        let tlb = l_tlb_mut!(self, layout);
        if tlb.region_mapped(layout.heap_end, size) {
            // Cannot extend the region since the address space after the region is already mapped
            Err(MemoryError::AlreadyMapped)?
        }

        let physical_region = reserve_first_free_physical_region(
            self.unused_physical_sets
                .get_mut(layout.unused_physical_memory)
                .expect("Emulator Error! The unused set must exist"),
            size,
        )?;

        debug!("Increasing heap with physical region {:?}", physical_region);
        let result = tlb.try_map(
            layout.heap_end,
            physical_region.start,
            size,
            Permission::READ | Permission::WRITE,
            MappingType::PrivateAnonymous,
            Some("Heap".to_string()),
        );
        result.map_err(|e| {
            self.unused_physical_sets
                .get_mut(layout.unused_physical_memory)
                .expect("Emulator Error! The unused physical set must exist")
                .insert(physical_region);
            e
        })?;

        layout.heap_end += size;

        Ok(())
    }

    pub(crate) fn map_data(
        &mut self,
        mmu: &mut impl MMU,
        task_id: TaskId,
        vm_range: Range<EVAddr>,
        data: &[u8],
        perms: Permission,
    ) -> Result<(), MemoryError> {
        let start = helpers::align4k_down(vm_range.start);
        let size = helpers::align4k_up(vm_range.end) - start;
        if data.len() > size {
            todo!()
        }

        self.map_fixed(task_id, start, size, perms, MappingType::PrivateAnonymous)?;
        self.accessor(mmu, task_id)
            .write_bytes_unchecked(vm_range.start, data)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn map_file(
        &mut self,
        mmu: &mut impl MMU,
        fd_map: &mut FileDescriptorMap<FD, TaskId, FileDescriptorData>,
        task_id: &TaskId,
        vm_range: Range<EVAddr>,
        ifd: &InternalFD,
        file_range: Range<usize>,
        perms: Permission,
    ) -> Result<(), MemoryError> {
        let start = helpers::align4k_down(vm_range.start);
        let size = helpers::align4k_up(vm_range.end) - start;
        if file_range.end - file_range.start > size {
            todo!()
        }
        let mut file = fd_map
            .get_mut_internal(task_id, ifd)
            .ok_or(MemoryError::Unknown)?;
        let file = &mut *file;
        let data = file
            .pread(file_range.start as u64, (file_range.end - file_range.start) as u64)
            .map_err(|_| MemoryError::Unknown)?;

        self.map_fixed(*task_id, start, size, perms, MappingType::PrivateFile(*ifd))?;
        self.accessor(mmu, *task_id)
            .write_bytes_unchecked(vm_range.start, data.as_slice())?;
        Ok(())
    }

    pub(crate) fn mmap(
        &mut self,
        task_id: TaskId,
        mut address_hint: EVAddr,
        size: usize,
        permissions: Permission,
        mapping_type: MappingType,
    ) -> Result<EVAddr, MemoryError> {
        let layout = layout!(self, task_id);
        // Clamp mmap address to map_base
        address_hint = max(address_hint, layout.map_base);
        self.map_fixed(
            task_id,
            l_tlb!(self, layout).find_free_space(address_hint, size)?,
            size,
            permissions,
            mapping_type,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn remap(
        &mut self,
        mmu: &mut impl MMU,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
        new_address: EVAddr,
        new_size: usize,
        flags: MREMAP_FLAGS,
    ) -> Result<EVAddr, MemoryError> {
        let size_aligned = helpers::align4k_up(size);

        if flags.contains(MREMAP_FLAGS::MREMAP_FIXED) {
            if !flags.contains(MREMAP_FLAGS::MREMAP_MAYMOVE) {
                Err(MemoryError::InvalidArguments)?
            }
            if address < new_address && address + size_aligned > new_address {
                Err(MemoryError::MappingOverlaps)?
            }

            if new_address < address && new_address + new_size > address {
                Err(MemoryError::MappingOverlaps)?
            }
            let new_size_aligned = helpers::align4k_up(new_size);

            let mappings = tlb!(self, task_id).get_mappings_copied(address, address + size)?;

            let mut first_mapping = mappings.last().unwrap();
            for mapping in mappings.iter().rev() {
                // All mappings must have the same mapping type.
                if first_mapping.mapping_type != mapping.mapping_type {
                    Err(MemoryError::InvalidArguments)?
                }
                // All mapping must have the same permissions.
                if first_mapping.permissions != mapping.permissions {
                    Err(MemoryError::InvalidPermission)?
                }
            }

            let unmap = if flags.contains(MREMAP_FLAGS::MREMAP_DONTUNMAP) {
                if first_mapping.mapping_type != MappingType::PrivateAnonymous {
                    Err(MemoryError::InvalidArguments)?
                }
                false
            } else {
                true
            };

            // Copy the mapping data before doing anything else
            let mapping_data = self.accessor(mmu, task_id).read_bytes(address, size)?;
            // Unmap any existing mappings from the new address area as specified by the man pages
            self.unmap(task_id, new_address, new_size_aligned);
            // Map in the new area
            self.map_fixed(
                task_id,
                new_address,
                new_size_aligned,
                first_mapping.permissions,
                first_mapping.mapping_type,
            )?;
            // Copy over the data
            self.accessor(mmu, task_id)
                .write_bytes(new_address, mapping_data.as_slice())?;
            // Unmap the old area
            if unmap {
                self.unmap(task_id, address, size_aligned)?;
            }
            Ok(new_address)
        } else {
            // todo: try resizing mapping by either shrinking the mapping or adding a new mapping directly after this mapping
            let err = MemoryError::AlreadyMapped; // Result of trying to resize
                                                  // todo: On already mapped error: map in free region and copy there. then unmap old, if !flags.contains(MREMAP_DONTUNMAP)
            todo!()
        }
    }

    pub(crate) fn protect(
        &mut self,
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        tlb_mut!(self, task_id).protect(addr, helpers::align4k_up(size), permissions)
    }

    pub(crate) fn unmap(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        if !is4kaligned(address) || !is4kaligned(size) {
            Err(MemoryError::InvalidAlignment)?
        }
        if size == 0 {
            Err(MemoryError::ZeroSize)?
        }

        let start = helpers::align4k_down(address);

        let layout = layout!(self, task_id);
        let tlb = l_tlb_mut!(self, layout);

        let free_physical_regions = tlb.unmap(address, size)?;

        free_physical_regions
            .into_iter()
            .for_each(|physical_region| {
                self.unused_physical_sets
                    .get_mut(layout.unused_physical_memory)
                    .expect("Emulator Error! The unused physical set must exist")
                    .insert(physical_region);
            });

        Ok(())
    }

    /// Maps the virtual memory region address..address + start to the first free physical region.
    /// Will fail if any address inside the virtual memory region is already mapped.
    pub(crate) fn map_fixed(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
        permissions: Permission,
        mapping_type: MappingType,
    ) -> Result<EVAddr, MemoryError> {
        if !helpers::is4kaligned(address) || !helpers::is4kaligned(size) {
            Err(MemoryError::InvalidAlignment)?
        }

        if size == 0 {
            Err(MemoryError::ZeroSize)?
        }

        let layout = layout_mut!(self, task_id);
        if l_tlb!(self, layout).region_mapped(address, size) {
            Err(MemoryError::AlreadyMapped)?
        }

        let physical_region = reserve_first_free_physical_region(
            self.unused_physical_sets
                .get_mut(layout.unused_physical_memory)
                .expect("Emulator Error! The unused physical memory index should point to a valid entry"),
            size,
        )?;
        // TODO: This will leak physical regions if try_map fails.
        tlb_mut!(self, task_id).try_map(address, physical_region.start, size, permissions, mapping_type, None)?;

        Ok(address)
    }
}

/// Helper function to get the first physical region that is large enough
/// to fit `size` bytes. If no appropriate region can be found, an error is returned.
///
/// If a region large enough to fit the requested block is found, the region is split,
/// and the first part is returned.
/// # Arguments
///
/// * `memory`: A set of `PhysicalRegion` objects to search
/// * `size`: The size of the requested memory area
fn reserve_first_free_physical_region(
    memory: &mut BTreeSet<PhysicalRegion>,
    size: usize,
) -> Result<PhysicalRegion, MemoryError> {
    debug!("Unused physical: {:?}", memory);
    let start = memory
        .iter()
        .find(|region| size <= region.size())
        .map(|r| r.start)
        .ok_or(MemoryError::OutOfMemory)?;
    let (front, back) = memory.take(&start).unwrap().split(size);
    back.map(|b| memory.insert(b));
    front.ok_or(MemoryError::Unknown)
}

/// Helper function to get the last physical region that is large enough
/// to fit `size` bytes. If no appropriate region can be found, an error is returned.
///
/// If a region large enough to fit the requested block is found, the region is split,
/// and the last part is returned.
/// # Arguments
///
/// * `memory`: A set of `PhysicalRegion` objects to search
/// * `size`: The size of the requested memory area
fn reserve_last_free_physical_region(
    memory: &mut BTreeSet<PhysicalRegion>,
    size: usize,
) -> Result<PhysicalRegion, MemoryError> {
    debug!("Unused physical: {:?}", memory);
    let start = memory
        .iter()
        .rev()
        .find(|region| size <= region.size())
        .map(|r| r.start)
        .ok_or(MemoryError::OutOfMemory)?;
    let (front, back) = memory.take(&start).unwrap().split_back(size);
    front.map(|b| memory.insert(b));
    back.ok_or(MemoryError::Unknown)
}
