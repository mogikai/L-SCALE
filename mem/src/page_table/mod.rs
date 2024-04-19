use crate::physical::PageId;
use crate::PAGE_SIZE;
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

struct PageTableEntry {
    page_id: PageId,
    perms: [Permission; PAGE_SIZE],
}

#[derive(Clone, Debug)]
pub struct PageTable {
    table: imbl::OrdMap<EVAddr, PageId>,
}

impl PageTable {
    pub fn new() -> Self {
        Self {
            table: Default::default(),
        }
    }

    /// Clears the entire page table
    pub fn flush(&mut self) {
        self.table.clear()
    }

    /// Map the address to the provided page id. If the address was already mapped,
    /// the previous page id is returned. The TLB will have to be flushed / updated for the old address.
    pub fn insert(
        &mut self,
        addr: EVAddr,
        page: PageId,
    ) -> Option<PageId> {
        self.table.insert(addr, page)
    }

    pub fn clear(
        &mut self,
        addr: EVAddr,
    ) {
        self.table.remove(&addr);
    }

    pub fn translate(
        &self,
        addr: EVAddr,
    ) -> Result<PageId, MemoryError> {
        addr.align_down(PAGE_SIZE as u64);
        self.table.get(&addr).copied().ok_or(MemoryError::PageFault)
    }
}
