use generational_arena::Index;
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
use crate::emulator::system::memory::tlb::Tlb;

/// Each Process/Thread has its own memory layout in the virtual address space
/// The mapping from virtual address to physical address is managed in the (shared) TLB
/// The TLB may be shared between multiple threads.
/// A new process usually has a clone of the tlb from the process it originated from.
#[derive(Debug, Copy, Clone)]
pub(super) struct Layout {
    pub stack_end: EVAddr,
    pub max_stack: usize,
    pub heap_base: EVAddr,
    pub heap_end: EVAddr,
    pub map_base: EVAddr,
    pub tlb: Index,
    /// A set of unused physical memory regions, shared between threads.
    /// TODO: Might make sense to sort the regions into buckets of different sizes for optimization
    pub unused_physical_memory: Index,
}

impl Layout {
    pub fn new(
        stack_base: EVAddr,
        max_stack: usize,
        heap_base: EVAddr,
        map_base: EVAddr,
        unused_physical_memory: Index,
        tlb: Index,
    ) -> Self {
        Self {
            stack_end: stack_base,
            max_stack,
            heap_base,
            heap_end: heap_base,
            map_base,
            unused_physical_memory,
            tlb,
        }
    }
}
