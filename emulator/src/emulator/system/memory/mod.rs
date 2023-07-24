use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, BitAnd, Not, Range};

use bitflags::Flags;
use byteorder::ByteOrder;

pub use accessor::MemoryAccessor;
use layout::Layout;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU, MREMAP_FLAGS};
// TODO: Make TaskId generic
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::rw_interface::DirectMemoryWrite;
pub use crate::emulator::system::memory::tlb::mapping::MappingType;
pub use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

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
            .ok_or(MemoryError::LayoutNotFound)
    };
}

macro_rules! layout_mut {
    ($self:ident, $task_id:ident) => {
        $self
            .layouts
            .get_mut(&$task_id)
            .ok_or(MemoryError::LayoutNotFound)
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

    /// The end of the allocated Emulated Physical address space.
    /// New processes are placed here and the end is then extended to after the new process.
    physical_end: EPAddr,

    /// The size of emulated physical memory that is allocated per process.
    process_mem_size: usize,

    phantom_data: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> Default for Memory<Endianness> {
    fn default() -> Self {
        Memory::new()
    }
}

impl<Endianness: ByteOrder> Memory<Endianness> {
    pub fn new() -> Self {
        Self {
            layouts: HashMap::new(),
            physical_end: helpers::align4k_down(EPAddr::from(0x8_0000_0000)),
            process_mem_size: helpers::align4k_down(1024u64 * 1024u64 * 1024u64) as usize, // 1 GiB
            phantom_data: Default::default(),
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

    pub fn new_process(
        &mut self,
        mmu: &mut impl MMU,
        task_id: TaskId,
    ) -> Result<(), MemoryError> {
        if self.layouts.contains_key(&task_id) {
            Err(MemoryError::ProcessAlreadyExists)?
        }

        let mut layout = Layout::new(
            0x0000_7fff_ffff_0000.into(),
            10 * 1024 * 1024, // 10 MiB
            0x0000_5000_0000_0000.into(),
            0x0000_3000_0000_0000.into(),
            PhysicalRegion::new(self.physical_end, self.physical_end + self.process_mem_size),
        );

        // stack
        let stack_physical_region = layout.reserve_last_free_physical_region(layout.max_stack)?;

        layout.tlb.borrow_mut().try_map(
            layout.stack_end - layout.max_stack,
            stack_physical_region.start,
            layout.max_stack,
            Permission::READ | Permission::WRITE,
            MappingType::PrivateAnonymous,
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

    pub fn translate_vaddr(
        &self,
        task_id: TaskId,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        layout!(self, task_id)?
            .tlb
            .borrow()
            .translate_virtual_address(address)
    }

    pub fn translate_vaddr_range(
        &self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<TLBEntry>, MemoryError> {
        layout!(self, task_id)?
            .tlb
            .borrow()
            .translate_virtual_addresses(address, size)
    }

    pub(crate) fn stack_base(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        Ok(layout!(self, task_id)?.stack_end)
    }

    pub(crate) fn stack_size(
        &self,
        task_id: TaskId,
    ) -> Result<usize, MemoryError> {
        Ok(layout!(self, task_id)?.max_stack)
    }

    pub(crate) fn heap_end(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        Ok(layout!(self, task_id)?.heap_end())
    }

    pub(crate) fn realloc_heap(
        &mut self,
        task_id: TaskId,
        new_heap_end: EVAddr,
    ) -> Result<(), MemoryError> {
        let layout = layout_mut!(self, task_id)?;

        if new_heap_end < layout.heap_base {
            Err(MemoryError::Unknown)?
        }

        let size: usize = helpers::align4k_up(new_heap_end - layout.heap_base);
        if size != 0 {
            layout.increase_heap(size);
        }

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

        self.map_fixed(task_id, start, size, Permission::ALL, MappingType::PrivateAnonymous)?;
        self.accessor(mmu, task_id)
            .write_direct(vm_range.start, data)?;
        self.protect(task_id, start, size, perms)
    }

    pub(crate) fn mmap(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
        permissions: Permission,
        mapping_type: MappingType,
    ) -> Result<EVAddr, MemoryError> {
        self.map_fixed(
            task_id,
            layout!(self, task_id)?.find_free_virtual_memory(address, size)?,
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
                Err(MemoryError::Overlap)?
            }

            if new_address < address && new_address + new_size > address {
                Err(MemoryError::Overlap)?
            }
            let new_size_aligned = helpers::align4k_up(new_size);

            let mappings = layout!(self, task_id)?
                .tlb
                .borrow()
                .get_mappings_copied(address, address + size)?;

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
        &self,
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        layout!(self, task_id)?
            .tlb
            .borrow_mut()
            .protect(addr, helpers::align4k_up(size), permissions)
    }

    pub(crate) fn unmap(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        if !helpers::is4kaligned(address) || !helpers::is4kaligned(size) {
            Err(MemoryError::NotAligned)?
        }
        if size == 0 {
            Err(MemoryError::ZeroSize)?
        }

        let start = helpers::align4k_down(address);
        let layout = layout_mut!(self, task_id)?;

        let free_physical_regions = layout.tlb.borrow_mut().unmap(address, size)?;

        free_physical_regions
            .into_iter()
            .try_for_each(|physical_region| layout.add_physical_region(physical_region))?;

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

        let layout = layout_mut!(self, task_id)?;
        if layout.tlb.borrow_mut().region_mapped(address, size) {
            Err(MemoryError::AlreadyMapped)?
        }

        let physical_region = layout.reserve_first_free_physical_region(size)?;
        // TODO: This will leak physical regions if try_map fails.
        layout
            .tlb
            .borrow_mut()
            .try_map(address, physical_region.start, size, permissions, mapping_type)?;

        Ok(address)
    }
}
