pub mod restorable;

use crate::page_table::PageTable;
use crate::physical::{
    Page,
    PageId,
    PhysicalMemory,
};
use crate::tlb::tlb::TLB;
use crate::{
    PageOffset,
    PAGE_SIZE,
};
use mogi_system::components::mmu::{
    MemoryAction,
    MmuResult,
    PageFaultCallback,
    Permission,
    MMU,
};
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::cell::RefCell;
use std::cmp::min;
use std::fmt::{
    Debug,
    Formatter,
};
use std::rc::Rc;
use tracing::debug;

pub type MemResult<T> = Result<T, MemoryError>;

#[derive(Clone, Debug)]
pub struct MemoryContext {
    /// The tlb is stored behind a pointer, since it needs to be passed to the JIT context.
    tlb: Box<TLB>,
    page_table: PageTable,
}

impl MemoryContext {
    fn new() -> Self {
        Self {
            tlb: Box::new(TLB::new()),
            page_table: PageTable::new(),
        }
    }

    fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    fn page_table_mut(&mut self) -> &mut PageTable {
        &mut self.page_table
    }
}

pub struct MogiMMU {
    /// The currently active memory context. This can be swapped out for example when performing multitasking.
    /// This way, each process can have its own tlb and page table.
    memory_context: MemoryContext,
    physical_memory: PhysicalMemory,
    // TODO this is bad. find better solution for ownership
    page_fault_callback: Rc<RefCell<PageFaultCallback<MemoryContext>>>,
}

impl Debug for MogiMMU {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("MMU")
            .field("memory_context", &self.memory_context)
            .field("physical_memory", &self.physical_memory)
            .finish()
    }
}

impl MogiMMU {
    pub fn new() -> Self {
        Self {
            memory_context: MemoryContext::new(),
            physical_memory: PhysicalMemory::new(1_000_000),
            page_fault_callback: Rc::new(RefCell::new(Box::new(|_, _, _| ()))),
        }
    }

    pub fn debug_tlb(&self) {
        debug!("{:?}", self.memory_context.tlb);
    }

    pub fn physical_memory_mut(&mut self) -> &mut PhysicalMemory {
        &mut self.physical_memory
    }

    pub fn tlb_ptr(&mut self) -> *mut TLB {
        self.memory_context.tlb.as_mut()
    }

    /// Resolves the provided EV address to a page id. If no entry is found in the page table,
    /// the OS is queried, and if a mapping is returned, it is inserted in the page table.
    /// TODO: Remove and use resolve_to_page, and move check_permissions to page resolving
    /// such that we can fault on invalid permissions, and get the newly mapped page.
    fn resolve_to_page_id(
        &mut self,
        addr: EVAddr,
        action: MemoryAction,
    ) -> Result<PageId, MemoryError> {
        self.memory_context.page_table.translate(addr).or_else(|_| {
            let callback = self.page_fault_callback.clone();
            let mut callback = callback.borrow_mut();
            callback(self, addr, action);
            self.memory_context.page_table.translate(addr)
        })
    }

    fn resolve_to_page(
        &mut self,
        addr: EVAddr,
        action: MemoryAction,
    ) -> Result<&Page, MemoryError> {
        let id = self
            .memory_context
            .page_table
            .translate(addr)
            .or_else(|_| {
                let callback = self.page_fault_callback.clone();
                let mut callback = callback.borrow_mut();
                callback(self, addr, action);
                self.memory_context.page_table.translate(addr)
            })?;

        self.physical_memory.page_by_id(id)

        // let result = check_permissions(page...);
        // if let Err(e) = result {
        //      let paddr = page_fault_callback()?;
        //      let page_id = self.physical_memory.page_id(paddr);
        //      self.physical_memory.page_by_id(page_id)
        // } else {
        //      page
        // }
    }

    // pub fn insert_page_table_entry(
    //     &mut self,
    //     addr: EVAddr,
    //     paddr: EPAddr,
    //     permissions: Permission,
    // ) -> Result<(), MemoryError> {
    //     let page_id = self.physical_memory.page_id(paddr)?;
    //     self.memory_context.page_table.insert(addr, page_id)?;
    //     self.physical_memory
    //         .page_by_id(page_id)?
    //         .permissions_mut()
    //         .iter_mut()
    //         .for_each(|p| *p = permissions);
    //     Ok(())
    // }

    fn read_internal(
        &mut self,
        mut addr: EVAddr,
        buffer: &mut [u8],
        action: MemoryAction,
    ) -> Result<(), MemoryError> {
        let size = buffer.len();
        // TODO: Use interior mutability to make read accept &self?
        let mut i = 0;
        let mut len = size;
        // Read in page sized chunks
        while len > 0 {
            let page_id = self.resolve_to_page_id(addr, action)?;
            let page = if let MemoryAction::FETCH = action {
                let page = self.physical_memory.page_by_id_mut(page_id)?;
                page.executed = true;
                &*page
            } else {
                self.physical_memory.page_by_id(page_id)?
            };
            populate_tlb(&mut self.memory_context.tlb, addr, page, action);
            let size = min(len, PAGE_SIZE - addr.offset());
            let page_start = addr.offset();
            let page_end = page_start + size;
            check_permissions(&page.permissions()[page_start..page_end], action.required_permissions())?;
            buffer[i..i + size].copy_from_slice(&page.data()[page_start..page_end]);
            i += size;
            addr += size;
            len -= size;
        }
        Ok(())
    }

    fn read_internal_physical(
        &self,
        mut addr: EPAddr,
        buffer: &mut [u8],
        action: MemoryAction,
    ) -> Result<(), MemoryError> {
        let size = buffer.len();
        // TODO: Use interior mutability to make read accept &self?
        let mut i = 0;
        let mut len = size;
        // Read in page sized chunks
        while len > 0 {
            // TODO: This could maybe be optimized a bit, so we dont have to look up each page individually
            let page = self.physical_memory.page(addr)?;
            let size = min(len, PAGE_SIZE - addr.offset());
            let page_start = addr.offset();
            let page_end = page_start + size;
            check_permissions(&page.permissions()[page_start..page_end], action.required_permissions())?;
            buffer[i..i + size].copy_from_slice(&page.data()[page_start..page_end]);
            i += size;
            addr += i;
            len -= size;
        }
        Ok(())
    }
}

// main trait implementation
impl MMU for MogiMMU {
    type MemoryContext = MemoryContext;

    fn new_context(&self) -> MemoryContext {
        MemoryContext::new()
    }

    fn clone_context(&self) -> MemoryContext {
        self.memory_context.clone()
    }

    fn load_context(
        &mut self,
        context: &MemoryContext,
    ) -> MemoryContext {
        std::mem::replace(&mut self.memory_context, context.clone())
    }

    fn flush_vaddr(
        &mut self,
        addr: EVAddr,
        len: usize,
    ) -> MmuResult<MemoryError> {
        todo!()
    }

    fn flush_all(&mut self) -> MmuResult<()> {
        self.memory_context.page_table.flush();
        self.memory_context.tlb.flush();

        Ok(())
    }

    fn set_page_table_entry(
        &mut self,
        addr: EVAddr,
        paddr: EPAddr,
        permissions: Permission,
    ) -> MmuResult<()> {
        let page_id = self.physical_memory.page_id(paddr)?;
        self.memory_context.page_table.insert(addr, page_id); // override existing
        self.physical_memory
            .page_by_id_mut(page_id)?
            .permissions_mut()
            .iter_mut()
            .for_each(|p| *p = permissions);

        Ok(())
    }

    fn clear_page_table_entry(
        &mut self,
        addr: EVAddr,
    ) -> MmuResult<()> {
        self.memory_context.page_table.clear(addr);
        self.memory_context.tlb.remove(addr);

        Ok(())
    }

    fn read_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        self.read_internal_physical(addr, buffer, MemoryAction::READ)
    }

    fn write_physical(
        &mut self,
        addr: EPAddr,
        data: &[u8],
    ) -> MmuResult<()> {
        let mut addr = addr;
        let mut i = 0;
        let mut len = data.len();
        // Write in page sized chunks
        while len > 0 {
            let page = self.physical_memory.page_mut(addr)?;
            let size = min(len, PAGE_SIZE - addr.offset());
            let page_start = addr.offset();
            let page_end = page_start + size;
            check_permissions(&page.permissions()[page_start..page_end], MemoryAction::WRITE.required_permissions())?;
            page.data_mut()[page_start..page_end].copy_from_slice(&data[i..i + size]);
            i += size;
            addr += size;
            len -= size;
        }
        Ok(())
    }

    fn fetch_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal_physical(addr, buffer, MemoryAction::FETCH)
    }

    fn read(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        self.read_internal(addr, buffer, MemoryAction::READ)
    }

    fn write(
        &mut self,
        addr: EVAddr,
        data: &[u8],
    ) -> MmuResult<()> {
        let mut i = 0;
        let mut addr = addr;
        let mut len = data.len();
        // Write in page sized chunks
        while len > 0 {
            let page_id = self.resolve_to_page_id(addr, MemoryAction::WRITE)?;
            let page = self.physical_memory.page_by_id_mut(page_id)?;
            let size = min(len, PAGE_SIZE - addr.offset());
            let page_start = addr.offset();
            let page_end = page_start + size;
            check_permissions(&page.permissions()[page_start..page_end], MemoryAction::WRITE.required_permissions())?;
            page.data_mut()[page_start..page_end].copy_from_slice(&data[i..i + size]);
            populate_tlb(&mut self.memory_context.tlb, addr, page, MemoryAction::WRITE);
            i += size;
            addr += size;
            len -= size;
        }

        Ok(())
    }

    fn fetch(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal(addr, buffer, MemoryAction::FETCH)
    }

    fn alloc_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        self.physical_memory
            .alloc(addr, size, Permission::ALL)
            .map(|_| ())
    }

    fn free_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        self.physical_memory.free(addr, size)
    }

    fn set_page_fault_callback(
        &mut self,
        callback: PageFaultCallback<MemoryContext>,
    ) -> Result<(), MemoryError> {
        self.page_fault_callback = Rc::new(RefCell::new(callback));

        Ok(())
    }
}

fn populate_tlb(
    tlb: &mut TLB,
    addr: EVAddr,
    page: &Page,
    action: MemoryAction,
) {
    match action {
        MemoryAction::READ => tlb.insert_read(addr, page.tlb_entry_ptr()),
        MemoryAction::WRITE => tlb.insert_write(addr, page.tlb_entry_ptr()),
        MemoryAction::FETCH => tlb.remove(addr),
        _ => (),
    }
}

fn check_permissions(
    permissions: &[Permission],
    required_permissions: Permission,
) -> Result<(), MemoryError> {
    if permissions
        .iter()
        .find(|&&perm| !perm.contains(required_permissions))
        .is_some()
    {
        Err(MemoryError::InvalidPermission)?
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::mmu::MogiMMU;
    use crate::PAGE_SIZE;
    use addr_align::Align;
    use mogi_system::components::mmu::Permission;
    use mogi_utilities::addresses::{
        EPAddr,
    };
    use mogi_utilities::addresses::EVAddr;

    #[test]
    fn test() {
        let mut mmu = MogiMMU::new();
        mmu.alloc_physical(0x8000u64.into(), 10000.align_up(PAGE_SIZE)).unwrap();
        // 1 to 1 mapping
        mmu.set_page_fault_callback(Box::new(|mmu, addr, action| {
            mmu.set_page_table_entry(addr, EPAddr::from(u64::from(addr)), Permission::ALL);
        }));
        mmu.write(0x8ff0u64.into(), &[b'A'; 8000]).unwrap();
        let mut buffer = [0u8; 8000];
        mmu.read(0x8ff0u64.into(), &mut buffer).unwrap();
        for byte in buffer {
            assert_eq!(byte, b'A');
        }
        let result = mmu.physical_memory.page(0x8000u64.into());
        println!("{:?}", result);
        let result = mmu.physical_memory.page(0x9000u64.into());
        println!("{:?}", result);
    }
}
