use std::cell::RefCell;
use std::ops::{Not, Range};
use std::rc::Rc;

use im_rc::HashMap;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr, EVA_NULL};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU, MREMAP_FLAGS};
use crate::emulator::system::memory::tlb::{TLBEntry, TLB};

pub mod tlb;

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct LayoutID {
    pid: u32,
    tid: u32,
}

impl LayoutID {
    pub fn new(
        pid: u32,
        tid: u32,
    ) -> Self {
        Self { pid, tid }
    }
}

/// Each Process/Thread has its own memory layout in the virtual address space
/// The mapping from virtual address to physical address is managed in the (shared) TLB
/// The TLB may be shared between multiple threads.
/// A new process usually has a clone of the tlb from the process it originated from.
#[derive(Clone)]
struct Layout {
    stack_base: EVAddr,
    max_stack: usize,
    heap_base: EVAddr,
    map_base: EVAddr,
    tlb: Rc<RefCell<TLB>>,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            stack_base: EVA_NULL,
            max_stack: 0,
            heap_base: EVA_NULL,
            map_base: EVA_NULL,
            tlb: Rc::new(RefCell::new(TLB::new())),
        }
    }
}

const MASK4K: u64 = 4 * (1 << 10) - 1;

fn align4k_up(address: EVAddr) -> EVAddr {
    (address + MASK4K) & !EVAddr::from(MASK4K)
}

fn align4k_down(address: EVAddr) -> EVAddr {
    address & !EVAddr::from(MASK4K)
}

fn is4kaligned(address: EVAddr) -> bool {
    (address & EVAddr::from(MASK4K)) == EVA_NULL
}

/// Contains the memory layout of the current process task. Each process has its own copy of the memory layout.
/// Multiple virtual addresses may be mapped to the same physical address.
/// The Memory layout may be shared between tasks (i.e. threads) (Mostly only the VTLB)
pub struct Memory {
    /// The virtual address layout of each process/thread (stack, heap, mappings, etc.)
    /// and the VTLB containing the mappings from emulated virtual addresses to emulated physical addresses.
    layouts: HashMap<LayoutID, Layout>,

    /// The end of the allocated Emulated Physical address space.
    /// New processes are placed here and the end is then extended to after the new process.
    physical_end: EPAddr,

    /// The size of emulated physical memory that is allocated per process.
    process_mem_size: usize,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            layouts: HashMap::new(),
            physical_end: EPAddr::from(0),
            process_mem_size: 0x2000000, //32 MiByte
        }
    }

    pub fn new_process(
        &mut self,
        mmu: &mut impl MMU,
        pid: u32,
        tid: u32,
    ) -> Result<(), MemoryError> {
        let mut layout = Layout::new();

        // heap
        layout.heap_base = EVAddr::from(0x55555555e000);
        layout.tlb.borrow_mut().try_insert(
            layout.heap_base,
            TLBEntry {
                physical_address: self.physical_end,
                size: 0x1000,
                permissions: Permission::READ | Permission::WRITE,
            },
        )?;

        // stack
        layout.stack_base = EVAddr::from(0x7ffff7fff000);
        layout.max_stack = 0x21000;
        layout.tlb.borrow_mut().try_insert(
            layout.stack_base - layout.max_stack,
            TLBEntry {
                physical_address: self.physical_end + self.process_mem_size - layout.max_stack,
                size: layout.max_stack,
                permissions: Permission::READ | Permission::WRITE,
            },
        )?;

        self.layouts
            .insert(LayoutID::new(pid, tid), layout)
            .ok_or(MemoryError::Unknown)?;
        mmu.mem_map(self.physical_end, self.process_mem_size, Permission::ALL)?;
        self.physical_end = self.physical_end + self.process_mem_size;
        Ok(())
    }

    pub(crate) fn stack_base(
        &self,
        pid: u32,
        tid: u32,
    ) -> Result<EVAddr, MemoryError> {
        let layout = self
            .layouts
            .get(&LayoutID { pid, tid })
            .ok_or(MemoryError::Unknown)?;
        Ok(layout.stack_base)
    }

    pub(crate) fn stack_size(
        &self,
        pid: u32,
        tid: u32,
    ) -> Result<usize, MemoryError> {
        let layout = self
            .layouts
            .get(&LayoutID { pid, tid })
            .ok_or(MemoryError::Unknown)?;
        Ok(layout.max_stack)
    }

    pub(crate) fn heap_end(
        &self,
        pid: u32,
        tid: u32,
    ) -> Result<EVAddr, MemoryError> {
        let layout = self
            .layouts
            .get(&LayoutID { pid: pid, tid: tid })
            .ok_or(MemoryError::Unknown)?;
        let tlb = layout.tlb.borrow();
        let (a, e) = tlb.get_mapping(layout.heap_base)?;
        Ok(a + e.size)
    }

    pub(crate) fn realloc_heap(
        &mut self,
        addr: EVAddr,
        pid: u32,
        tid: u32,
    ) -> Result<(), MemoryError> {
        let layout = self
            .layouts
            .get(&LayoutID { pid: pid, tid: tid })
            .ok_or(MemoryError::Unknown)?;
        if addr < layout.heap_base {
            return Err(MemoryError::Unknown);
        }
        let mut tlb = layout.tlb.borrow_mut();
        let (a, e) = tlb.get_mapping(layout.heap_base)?;
        if addr < a + e.size {
            tlb.realloc(a, addr - a)?;
            return Ok(());
        }
        if addr - a > 0x40000000 {
            return Err(MemoryError::Unknown);
        }
        tlb.realloc(a, addr - a)?;
        Ok(())
    }

    pub(crate) fn map_data(
        &mut self,
        cpu: &mut impl MMU,
        vm_range: Range<EVAddr>,
        data: &[u8],
        perms: Permission,
    ) -> Result<(), MemoryError> {
        // let start = align4k_down(vm_range.start);
        // let size = align4k_up(vm_range.end) - start;
        // if data.len() > size as usize {
        //     todo!()
        // }
        // if self.get_region(start).is_some() || self.get_region(start + size).is_some() {
        //     todo!()
        // }
        // self.fixed_mapping(start, size, perms, cpu)?;
        // self.write_internal(vm_range.start, data);
        // Ok(())
        todo!()
    }

    pub(crate) fn mmap(
        &mut self,
        addr: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<EVAddr, MemoryError> {
        todo!()
    }

    pub(crate) fn remap(
        &self,
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
        address: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        todo!()
    }

    pub(crate) fn unmap(
        &mut self,
        addr: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        todo!()
    }
}
