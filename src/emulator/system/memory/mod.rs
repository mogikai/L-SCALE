use std::cell::RefCell;
use std::cmp::max;
use std::ops::Range;
use std::rc::Rc;

use im_rc::HashMap;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr, EVA_NULL};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU, MREMAP_FLAGS};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::memory::tlb::{TLBEntry, TLB};

pub mod tlb;

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
    designated_physical_memory: Range<EPAddr>,
    tlb: Rc<RefCell<TLB>>,
}

impl Layout {
    pub fn new(
        stack_base: EVAddr,
        max_stack: usize,
        heap_base: EVAddr,
        map_base: EVAddr,
        designated_physical_memory: Range<EPAddr>,
    ) -> Self {
        Self {
            stack_base,
            max_stack,
            heap_base,
            map_base,
            designated_physical_memory,
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
    layouts: HashMap<TaskId, Layout>,

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
            process_mem_size: 1024 * 1024 * 1024, // 1 GiB
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

        let layout = Layout::new(
            0x0000_7fff_ffff_ffff.into(),
            10 * 1024 * 1024, // 10 MiB
            0x0000_5000_0000_0000.into(),
            0x0000_3000_0000_0000.into(),
            self.physical_end..(self.physical_end + self.process_mem_size),
        );

        // heap
        layout.tlb.borrow_mut().try_insert(
            layout.heap_base,
            TLBEntry {
                physical_address: layout.designated_physical_memory.start,
                size: 0x1000,
                permissions: Permission::READ | Permission::WRITE,
            },
        )?;

        // stack
        layout.tlb.borrow_mut().try_insert(
            layout.stack_base - layout.max_stack,
            TLBEntry {
                physical_address: layout.designated_physical_memory.end - layout.max_stack,
                size: layout.max_stack,
                permissions: Permission::READ | Permission::WRITE,
            },
        )?;

        mmu.mem_map(self.physical_end, self.process_mem_size, Permission::ALL)?;
        self.physical_end += self.process_mem_size;

        let old_layout = self.layouts.insert(task_id, layout);
        assert!(old_layout.is_none());
        Ok(())
    }

    pub(crate) fn stack_base(
        &self,
        task_id: TaskId,
    ) -> Result<EVAddr, MemoryError> {
        self.layouts
            .get(&task_id)
            .map(|layout| layout.stack_base)
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
                    .get_mapping(layout.heap_base)
                    .map(|(block_start, tlb_entry)| block_start + tlb_entry.size)
            })
    }

    pub(crate) fn realloc_heap(
        &mut self,
        task_id: TaskId,
        addr: EVAddr,
    ) -> Result<(), MemoryError> {
        let layout = self.layouts.get(&task_id).ok_or(MemoryError::Unknown)?;
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
        task_id: TaskId,
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
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<EVAddr, MemoryError> {
        todo!()
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
        address: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        todo!()
    }

    pub(crate) fn unmap(
        &mut self,
        task_id: TaskId,
        addr: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        todo!()
    }

    // pub fn find_space_for_region(
    //     &self,
    //     hint: EVAddr,
    //     size: u64,
    // ) -> u64 {
    //     let mut hint = max(hint, self.map_base);
    //     for region in self.mappings.range(hint..) {
    //         if region.contains(hint) || region.contains(hint + size - 1) {
    //             hint = region.vaddr + region.size as EVAddr;
    //         } else {
    //             break;
    //         }
    //     }
    //     hint
    // }
}
