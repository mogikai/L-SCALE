use std::cell::RefCell;
use std::cmp::max;
use std::collections::BTreeSet;
use std::iter::once;
use std::rc::Rc;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::system::memory::helpers::is4kaligned;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::tlb::mapping::MappingType;
use crate::emulator::system::memory::tlb::TLB;

/// Each Process/Thread has its own memory layout in the virtual address space
/// The mapping from virtual address to physical address is managed in the (shared) TLB
/// The TLB may be shared between multiple threads.
/// A new process usually has a clone of the tlb from the process it originated from.
#[derive(Clone, Debug)]
pub struct Layout {
    pub stack_end: EVAddr,
    pub max_stack: usize,
    pub heap_base: EVAddr,
    heap_end: EVAddr,
    pub map_base: EVAddr,
    pub tlb: Rc<RefCell<TLB>>,
    /// A set of unused physical memory regions, shared between threads.
    /// TODO: Might make sense to sort the regions into buckets of different sizes for optimization
    unused_physical_memory: Rc<RefCell<BTreeSet<PhysicalRegion>>>,
}

impl Layout {
    pub fn new(
        stack_base: EVAddr,
        max_stack: usize,
        heap_base: EVAddr,
        map_base: EVAddr,
        designated_physical_memory: PhysicalRegion,
    ) -> Self {
        Self {
            stack_end: stack_base,
            max_stack,
            heap_base,
            heap_end: heap_base,
            map_base,
            unused_physical_memory: Rc::new(RefCell::new(BTreeSet::from_iter(once(designated_physical_memory)))),
            tlb: Rc::new(RefCell::new(TLB::new())),
        }
    }

    pub fn reserve_first_free_physical_region(
        &mut self,
        size: usize,
    ) -> Result<PhysicalRegion, MemoryError> {
        let mut memory = self.unused_physical_memory.borrow_mut();
        let start = memory
            .iter()
            .find(|region| size <= region.size())
            .map(|r| r.start)
            .ok_or(MemoryError::OutOfMemory)?;
        let (front, back) = memory.take(&start).unwrap().split(size);
        back.map(|b| memory.insert(b));
        front.ok_or(MemoryError::Unknown)
    }

    pub fn reserve_last_free_physical_region(
        &mut self,
        size: usize,
    ) -> Result<PhysicalRegion, MemoryError> {
        let mut memory = self.unused_physical_memory.borrow_mut();
        let start = memory
            .iter()
            .rev()
            .find(|region| size <= region.size())
            .map(|r| r.start)
            .ok_or(MemoryError::OutOfMemory)?;
        let (front, back) = memory.take(&start).unwrap().split(size);
        front.map(|b| memory.insert(b));
        back.ok_or(MemoryError::Unknown)
    }

    /// Adds a currently unused physical region to the layouts set of free physical regions.
    /// The layout may merge existing adjacent regions into one large region.
    pub fn add_physical_region(
        &mut self,
        mut region: PhysicalRegion,
    ) -> Result<(), MemoryError> {
        // FIXME: Reusing physical regions currently causes a change in program behavior. Probably bug
        region.set_dirty();
        assert!(self.unused_physical_memory.borrow_mut().insert(region));
        Ok(())
    }

    /// Finds free space for a requested virtual memory region.
    /// The returned address is always larger than the layouts map_base.
    pub fn find_free_virtual_memory(
        &self,
        hint: EVAddr,
        size: usize,
    ) -> Result<EVAddr, MemoryError> {
        self.tlb
            .borrow()
            .find_free_space(max(hint, self.map_base), size)
    }

    pub fn increase_heap(
        &mut self,
        size: usize,
    ) -> Result<(), MemoryError> {
        if size == 0 || !is4kaligned(size) {
            Err(MemoryError::InvalidSize)?
        }

        if self.tlb.borrow().region_mapped(self.heap_end, size) {
            // Cannot extend the region since the address space after the region is already mapped
            Err(MemoryError::AlreadyMapped)?
        }

        let physical_region = self.reserve_first_free_physical_region(size)?;
        self.tlb.borrow_mut().try_map(
            self.heap_end,
            physical_region.start,
            size,
            Permission::READ | Permission::WRITE,
            MappingType::PrivateAnonymous,
        )?;
        self.heap_end += size;

        Ok(())
    }

    pub fn heap_end(&self) -> EVAddr {
        self.heap_end
    }
}
