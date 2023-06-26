use std::cmp::max;
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, BitAnd, Not, Range};

use byteorder::ByteOrder;

use layout::Layout;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr, EVA_NULL};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU, MREMAP_FLAGS};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::rw_interface::DirectMemoryWrite;
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

pub mod layout;
pub mod pregion;
pub mod rw_interface;
pub mod tlb;

const MASK4K: u64 = 4 * (1 << 10) - 1;

fn align4k_up<T>(address: T) -> T
where
    T: Add<T, Output = T> + Not<Output = T> + TryFrom<u64> + BitAnd<Output = T> + Copy,
    <T as TryFrom<u64>>::Error: Debug,
{
    let mask = T::try_from(MASK4K).expect("Address like type must be convertible from 64 bit values");
    (address + mask) & !mask
}

fn align4k_down<T>(address: T) -> T
where
    T: Not<Output = T> + TryFrom<u64> + BitAnd<Output = T>,
    <T as TryFrom<u64>>::Error: Debug,
{
    address & !T::try_from(MASK4K).expect("Address like type must be convertible from 64 bit values")
}

fn is4kaligned<T>(address: T) -> bool
where
    T: TryFrom<u64> + BitAnd<Output = T> + Eq + Copy,
    <T as TryFrom<u64>>::Error: Debug,
{
    let mask = T::try_from(0).expect("Address like type must be convertible from 64 bit values");
    (address & mask) == mask
}

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

pub struct MemoryAccessor<'a, Endianness: ByteOrder, MmuT: MMU> {
    memory: &'a mut Memory<Endianness>,
    mmu: &'a mut MmuT,
    task_id: TaskId,
}

impl<Endianness: ByteOrder, MmuT: MMU> MemoryAccessor<'_, Endianness, MmuT> {
    fn translate(
        &self,
        address: EVAddr,
    ) -> Result<EPAddr, MemoryError> {
        self.memory
            .translate_vaddr(self.task_id, address)
            .ok_or(MemoryError::NotMapped)
            .map(|tlb_entry| tlb_entry.physical_address)
    }

    pub fn read_bytes(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        self.mmu.mem_read_bytes(self.translate(address)?, size)
    }
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

impl<Endianness: ByteOrder> Memory<Endianness> {
    pub fn new() -> Self {
        Self {
            layouts: HashMap::new(),
            physical_end: align4k_down(EPAddr::from(0x8_0000_0000)),
            process_mem_size: align4k_down(1024u64 * 1024u64 * 1024u64) as usize, // 1 GiB
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
            500 * 1024 * 1024, // 10 MiB
            0x0000_5000_0000_0000.into(),
            0x0000_3000_0000_0000.into(),
            PhysicalRegion::new(self.physical_end, self.physical_end + self.process_mem_size),
        );

        // TODO: Allocate heap on the fly
        // heap
        // let heap_physical_region = layout.reserve_last_free_physical_region(layout.max_heap)?;
        // layout.tlb.borrow_mut().try_map(
        //     layout.heap_base,
        //     heap_physical_region.start,
        //     0x0000_5000_0000_0000.into()
        //     Permission::READ | Permission::WRITE,
        // )?;

        // stack

        let stack_physical_region = layout.reserve_last_free_physical_region(layout.max_stack)?;

        layout.tlb.borrow_mut().try_map(
            layout.stack_end - layout.max_stack,
            stack_physical_region.start,
            layout.max_stack,
            Permission::READ | Permission::WRITE,
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
    ) -> Option<TLBEntry> {
        let layout = self.layouts.get(&task_id);
        if layout.is_none() {
            trace!("Could not fetch layout for address translation of {address:?}");
        }
        let tlb_entry = layout.and_then(|layout| layout.tlb.borrow().translate_virtual_address(address).ok());
        trace!("Translation result for address {address:?} is {tlb_entry:?}");
        tlb_entry
    }

    pub(crate) fn stack_base(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        self.layouts
            .get(&task_id)
            .map(|layout| layout.stack_end)
            .ok_or(MemoryError::Unknown)
    }

    pub(crate) fn stack_size(
        &self,
        task_id: TaskId,
    ) -> Result<usize, MemoryError> {
        self.layouts
            .get(&task_id)
            .map(|layout| layout.max_stack)
            .ok_or(MemoryError::Unknown)
    }

    pub(crate) fn heap_end(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        self.layouts
            .get(&task_id)
            .ok_or(MemoryError::LayoutNotFound)
            .and_then(|layout| {
                layout
                    .tlb
                    .borrow()
                    .get_region(layout.heap_base)
                    .map(|region| region.v_start + region.size)
                    .or(Ok(layout.heap_base))
            })
    }

    pub(crate) fn realloc_heap(
        &mut self,
        task_id: TaskId,
        addr: EVAddr,
    ) -> Result<(), MemoryError> {
        let layout = self.layouts.get_mut(&task_id).ok_or(MemoryError::Unknown)?;
        if addr < layout.heap_base {
            Err(MemoryError::Unknown)?
        }
        let heap_base = layout.heap_base;
        trace!("get heap: {:#x}", u64::from(heap_base));
        let heapstart = match layout.tlb.borrow_mut().get_region(heap_base) {
            Ok(heap) => {
                Some(heap.v_start)
            },
            Err(_) => {
                None
            },
        };
        match heapstart {
            Some(heapstart) => {
                layout.tlb.borrow_mut().resize(heapstart, addr - heapstart)?;
                return Ok(());
            }
            None => {
                let size: usize = align4k_up(addr - heap_base);
                let physical_region = layout.reserve_first_free_physical_region(size)?;
                layout.tlb.borrow_mut().try_map(heap_base, physical_region.start, size, Permission::READ | Permission::WRITE)?;
                return Ok(());
            },
        }
    }

    pub(crate) fn map_data(
        &mut self,
        mmu: &mut impl MMU,
        task_id: TaskId,
        vm_range: Range<EVAddr>,
        data: &[u8],
        perms: Permission,
    ) -> Result<(), MemoryError> {
        let start = align4k_down(vm_range.start);
        let size = align4k_up(vm_range.end) - start;
        if data.len() > size as usize {
            todo!()
        }

        self.fixed_mapping(task_id, start, size, perms)?;
        self.accessor(mmu, task_id)
            .write_direct(vm_range.start, data)?;
        Ok(())
    }

    pub(crate) fn mmap(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<EVAddr, MemoryError> {
        let size = align4k_up(size);
        if !is4kaligned(address) {
            return Err(MemoryError::NotAligned);
        }
        if size == 0 {
            return Err(MemoryError::ZeroSize);
        }
        self.fixed_mapping(task_id, layout!(self, task_id)?.find_free_virtual_memory(address, size)?, size, permissions)
    }

    pub(crate) fn remap(
        &self,
        task_id: TaskId,
        address: EVAddr,
        size: u64,
        new_address: EVAddr,
        new_size: u64,
        flags: MREMAP_FLAGS,
    ) -> Result<EVAddr, MemoryError> {
        todo!()
    }

    pub(crate) fn protect(
        &self,
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        let start = align4k_down(addr);
        let size = align4k_up(size);
        let layout = self.layouts.get(&task_id).ok_or(MemoryError::Unknown)?;
        let mut tlb = layout.tlb.borrow_mut();
        let region = tlb.get_region(start)?;
        if addr + size > region.v_start + region.size {
            todo!("unmap multible regions");
        }
        if start > region.v_start {
            tlb.resize(region.v_start, start - region.v_start);
            tlb.try_map(start, region.p_start.unwrap() + (addr - region.v_start), size, permissions);
        } else {
            tlb.resize(region.v_start, size);
            tlb.setperms(region.v_start, permissions);
        }

        if start + size < region.v_start + region.size {
            //TODO check why Option<EPAddr> and maybe check better
            tlb.try_map(start + size, region.p_start.unwrap() + (addr - region.v_start) + size, region.v_start + region.size - (start + size), region.permissions)?;
        }
        return Ok(());
        todo!()
    }

    pub(crate) fn unmap(
        &mut self,
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        let start = align4k_down(addr);
        let size = align4k_up(size);
        let layout = self.layouts.get(&task_id).ok_or(MemoryError::Unknown)?;
        let mut tlb = layout.tlb.borrow_mut();
        let region = tlb.get_region(start)?;
        if addr + size > region.v_start + region.size {
            todo!("unmap multible regions");
        }
        if start > region.v_start {
            tlb.resize(region.v_start, start - region.v_start);
        } else {
            tlb.remove(region);
        }
        if start + size < region.v_start + region.size {
            //TODO check why Option<EPAddr> and maybe check better
            tlb.try_map(start + size, region.p_start.unwrap() + (addr - region.v_start) + size, region.v_start + region.size - (start + size), region.permissions)?;
        }
        return Ok(());
    }

    /// Maps the virtual memory region address..address + start to the first free physical region.
    /// Will fail if any address inside the virtual memory region is already mapped.
    fn fixed_mapping(
        &mut self,
        task_id: TaskId,
        address: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<EVAddr, MemoryError> {
        let layout = layout_mut!(self, task_id)?;
        if layout.tlb.borrow_mut().region_mapped(address, size) {
            Err(MemoryError::AlreadyMapped)?
        }

        let physical_region = layout.reserve_first_free_physical_region(size)?;
        // TODO: This will leak physical regions if try_map fails.
        layout
            .tlb
            .borrow_mut()
            .try_map(address, physical_region.start, size, permissions)?;

        Ok(address)
    }
}
