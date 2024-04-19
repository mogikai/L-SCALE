use crate::page_table::PageTable;
use crate::physical::{
    Page,
    PageId,
    PhysicalMemory,
};
use crate::tlb::tlb::TLB;
use crate::{
    PageOffset,
    MAX_PAGE_OFFSET,
    PAGE_SIZE,
};
use addr_align::Align;
use mogi_system::components::mmu::{
    MemoryAction,
    Permission,
};
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::cmp::min;
use std::fmt::{
    Debug,
    Formatter,
};
use tracing::{
    debug,
    error,
};

pub type PageFaultCallback = Box<dyn FnMut(EVAddr, MemoryAction) -> Result<EPAddr, MemoryError>>;

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

pub struct MMU {
    /// The currently active memory context. This can be swapped out for example when performing multitasking.
    /// This way, each process can have its own tlb and page table.
    memory_context: MemoryContext,
    physical_memory: PhysicalMemory,
    page_fault_callback: PageFaultCallback,
}

impl Debug for MMU {
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

impl MMU {
    pub fn new() -> Self {
        Self {
            memory_context: MemoryContext::new(),
            physical_memory: PhysicalMemory::new(1_000_000),
            page_fault_callback: Box::new(|_, _| Err(MemoryError::Unknown)),
        }
    }

    pub fn debug_tlb(&self) {
        debug!("{:?}", self.memory_context.tlb);
    }

    pub fn physical_memory(&self) -> &PhysicalMemory {
        &self.physical_memory
    }

    pub fn physical_memory_mut(&mut self) -> &mut PhysicalMemory {
        &mut self.physical_memory
    }

    pub fn new_memory_context(&self) -> MemoryContext {
        MemoryContext::new()
    }

    pub fn clone_memory_context(&self) -> MemoryContext {
        self.memory_context.clone()
    }

    pub fn swap_memory_context(
        &mut self,
        context: MemoryContext,
    ) -> MemoryContext {
        std::mem::replace(&mut self.memory_context, context)
    }

    pub fn set_page_fault_callback(
        &mut self,
        callback: PageFaultCallback,
    ) {
        self.page_fault_callback = callback
    }

    pub fn flush(&mut self) {
        self.memory_context.page_table.flush();
        self.memory_context.tlb.flush();
    }

    pub fn flush_tlb(&mut self) {
        self.memory_context.tlb.flush();
    }

    pub fn flush_page_table(&mut self) {
        self.memory_context.page_table.flush();
    }

    pub fn tlb_ptr(&mut self) -> *mut TLB {
        self.memory_context.tlb.as_mut()
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

    pub fn clear_page_table_entry(
        &mut self,
        addr: EVAddr,
    ) {
        self.memory_context.page_table.clear(addr);
        self.memory_context.tlb.remove(addr);
    }
}

// Read / Write functionality
impl MMU {
    pub fn write(
        &mut self,
        mut addr: EVAddr,
        data: &[u8],
    ) -> Result<(), MemoryError> {
        let mut i = 0;
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

    pub fn write_physical(
        &mut self,
        mut addr: EPAddr,
        data: &[u8],
    ) -> Result<(), MemoryError> {
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

    pub fn read(
        &mut self,
        mut addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal(addr, buffer, MemoryAction::READ)
    }

    pub fn fetch(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal(addr, buffer, MemoryAction::FETCH)
    }

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
            addr += i;
            len -= size;
        }
        Ok(())
    }

    pub fn read_physical(
        &self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal_physical(addr, buffer, MemoryAction::READ)
    }

    pub fn fetch_physical(
        &self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.read_internal_physical(addr, buffer, MemoryAction::FETCH)
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

impl MMU {
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
            // No entry in page table. Ask OS for mapping and look up the page id to the provided EP address.
            (self.page_fault_callback)(addr, action)
                .and_then(|paddr| self.physical_memory.page_id(paddr))
                .and_then(|page_id| {
                    // Add newly translated address into page table
                    self.memory_context.page_table.insert(addr, page_id);
                    Ok(page_id)
                })
        })
    }

    fn resolve_to_page(
        &mut self,
        addr: EVAddr,
        action: MemoryAction,
    ) -> Result<&Page, MemoryError> {
        let page = self
            .memory_context
            .page_table
            .translate(addr)
            .and_then(|id| self.physical_memory.page_by_id(id))
            .or_else(|_| {
                // No entry in page table. Ask OS for mapping and look up the page id to the provided EP address.
                (self.page_fault_callback)(addr, action)
                    .and_then(|paddr| self.physical_memory.page_id(paddr))
                    .and_then(|page_id| {
                        // Add newly translated address into page table
                        self.memory_context.page_table.insert(addr, page_id);
                        self.physical_memory.page_by_id(page_id)
                    })
            });
        // let result = check_permissions(page...);
        // if let Err(e) = result {
        //      let paddr = page_fault_callback()?;
        //      let page_id = self.physical_memory.page_id(paddr);
        //      self.physical_memory.page_by_id(page_id)
        // } else {
        //      page
        // }
        page
    }
}

#[cfg(test)]
mod test {
    use crate::mmu::MMU;
    use crate::PAGE_SIZE;
    use addr_align::Align;
    use mogi_system::components::mmu::Permission;
    use mogi_utilities::addresses::{
        EPAddr,
        EVAddr,
    };

    #[test]
    fn test() {
        let mut mmu = MMU::new();
        let _ = mmu
            .physical_memory
            .alloc(0x8000u64.into(), 10000.align_up(PAGE_SIZE), Permission::READ_WRITE)
            .unwrap();
        // 1 to 1 mapping
        mmu.set_page_fault_callback(Box::new(|addr, action| Ok(EPAddr::from(u64::from(addr)))));
        mmu.write(0x8ff0u64.into(), &[b'A'; 8000]).unwrap();
        let mut buffer = [0u8; 8000];
        let data = mmu.read(0x8ff0u64.into(), &mut buffer).unwrap();
        for byte in data {
            assert_eq!(byte, b'A');
        }
        let result = mmu.physical_memory.page(0x8000u64.into());
        println!("{:?}", result);
        let result = mmu.physical_memory.page(0x9000u64.into());
        println!("{:?}", result);
    }
}
