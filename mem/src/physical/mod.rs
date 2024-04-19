use crate::physical::mapping::{
    Contains,
    PhysicalMapping,
};
use crate::{
    PageOffset,
    PAGE_SIZE,
};
use addr_align::Align;
use mogi_system::components::mmu::Permission;
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};
use mogi_utilities::error::memory::MemoryError;
pub use page::Page;
use std::array;
use std::cmp::{
    max,
    min,
};
use std::fmt::{
    Debug,
    Formatter,
    Pointer,
};
use std::iter::once;

pub mod mapping;
pub mod page;

#[derive(Debug, Copy, Clone)]
pub struct PageId(usize);

#[derive(Clone)]
pub struct PhysicalMemory {
    zero_pages: [Page; Permission::ALL.bits() as usize + 1],
    pages: imbl::Vector<Page>,
    free_pages: imbl::Vector<PageId>,
    max_pages: usize,
    physical_address_space: imbl::OrdSet<PhysicalMapping>, // TODO: Better datastructure?
}

impl Debug for PhysicalMemory {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        let zero_pages = format!("<{} omitted>", self.zero_pages.len());
        let pages = format!("<{} omitted>", self.pages.len());
        f.debug_struct("PhysicalMemory")
            .field("zero_pages", &zero_pages)
            .field("pages", &pages)
            .field("free_pages", &self.free_pages)
            .field("max_pages", &self.max_pages)
            .field("physical_address_space", &self.physical_address_space)
            .finish()
    }
}

// MappingTable <-> PageTable <-> PhysicalMemory <-> TLB
// EV -> EP, HV(Page)
impl PhysicalMemory {
    pub fn new(max_pages: usize) -> Self {
        let mut slf = Self {
            zero_pages: array::from_fn(|_| Page::new()),
            pages: Default::default(),
            free_pages: Default::default(),
            max_pages,
            physical_address_space: Default::default(),
        };
        slf.zero_pages
            .iter_mut()
            .enumerate()
            .for_each(|(i, page)| page.set_permissions(Permission::from_bits(i as u8).unwrap()));
        slf
    }

    pub fn alloc(
        &mut self,
        addr: EPAddr,
        size: usize,
        permission: Permission,
    ) -> Result<impl Iterator<Item = &PageId>, MemoryError> {
        if !size.is_aligned(PAGE_SIZE) || !addr.is_aligned(PAGE_SIZE as u64) {
            Err(MemoryError::InvalidAlignment)?
        }
        let requested_pages = size / PAGE_SIZE;
        let reused_pages = min(self.free_pages.len(), requested_pages);
        // Reuse any old pages
        let mut page_ids = self
            .free_pages
            .slice(..min(requested_pages, self.free_pages.len()));
        for id in page_ids.iter() {
            self.pages[id.0].set_permissions(permission);
        }

        let requested_pages = requested_pages.saturating_sub(reused_pages);
        let first_new_id = self.pages.len();
        let page_prototype = self.create_page(permission);
        self.pages
            .extend((0..requested_pages).map(|_| page_prototype.clone()));
        page_ids.extend((first_new_id..self.pages.len()).map(PageId));
        self.physical_address_space
            .insert(PhysicalMapping::new(addr, size, page_ids));
        Ok(self
            .physical_address_space
            .get(&addr)
            .expect("We just inserted this")
            .page_ids
            .iter())
    }

    /// Allocate a single page for the specified address.
    /// The allocation will be lazy, if the permissions are either read only, or read/write.
    pub fn alloc_page(
        &mut self,
        addr: EPAddr,
        permission: Permission,
    ) -> Result<PageId, MemoryError> {
        if let Some(index) = self.free_pages.pop_back() {
            self.pages[index.0].set_permissions(permission);
            Ok(index)
        } else {
            if self.pages.len() >= self.max_pages {
                Err(MemoryError::OutOfMemory)?
            }

            self.pages.push_back(self.create_page(permission));
            Ok(PageId(self.pages.len() - 1))
        }
    }

    /// Create a new page by either cloning an existing page or creating a new one.
    /// If cloning an existing page, the data is reused and only allocated separately
    /// if a write access is performed.
    fn create_page(
        &self,
        permission: Permission,
    ) -> Page {
        self.zero_pages[permission.bits() as usize].clone()
    }

    fn get_physical_mapping_start_addresses(
        &self,
        start: EPAddr,
        end: EPAddr,
    ) -> Vec<EPAddr> {
        self.physical_address_space
            .range(..end)
            .rev()
            .take_while(move |m| start < m.end())
            .map(|m| m.start)
            .collect()
    }

    pub fn free(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        if !addr.is_aligned(PAGE_SIZE as u64) || !size.is_aligned(PAGE_SIZE) {
            Err(MemoryError::InvalidAlignment)?
        }
        self.get_physical_mapping_start_addresses(addr, addr + size)
            .into_iter()
            .rev()
            .for_each(|start| {
                // Remove the original mapping so we can modify it
                let mapping = self
                    .physical_address_space
                    .remove(&start)
                    .expect("The mapping must exist since the addresses were previously gathered from the collection");

                // Clamp cutout to mapping boundaries with max and min
                let start = max(addr, mapping.start);
                let size = min(size, mapping.size);
                let (front, middle, back) = mapping
                    .cutout(start, size)
                    .expect("This cutout operation should never fail if implemented correctly");

                self.free_pages.extend(middle.page_ids.iter().copied());

                // Reinsert remainders
                front.map(|m| self.physical_address_space.insert(m));
                back.map(|m| self.physical_address_space.insert(m));
            });
        Ok(())
    }

    // #[inline]
    // pub fn address_of(
    //     &self,
    //     vaddr: u64,
    //     index: crate::deprecated::physical::PageIndex,
    // ) -> PhysicalAddr {
    //     let base = (index.0 << OFFSET_BITS) as u64;
    //     let offset = vaddr & ((1_u64 << OFFSET_BITS) - 1);
    //     PhysicalAddr(base | offset)
    // }

    /// Allocate a copy of a page.
    pub fn clone_page(
        &mut self,
        index: PageId,
    ) -> Option<PageId> {
        todo!()
    }

    // /// Return mutable references to two distict pages
    // pub fn get_pair_mut(
    //     &mut self,
    //     a: crate::deprecated::physical::PageIndex,
    //     b: crate::deprecated::physical::PageIndex,
    // ) -> (&mut crate::deprecated::physical::Page, &mut crate::deprecated::physical::Page) {
    //     let end = self.allocated.len() as u32;
    //     assert!(a.0 != b.0 && a.0 < end && b.0 <= end);
    //
    //     // Safety: we have ensured that both indices are inbounds and are distinct.
    //     unsafe {
    //         let ptr = self.allocated.as_mut_ptr();
    //         (ptr.add(a.0 as usize).as_mut().unwrap(), ptr.add(b.0 as usize).as_mut().unwrap())
    //     }
    // }

    pub fn page(
        &self,
        addr: EPAddr,
    ) -> Result<&Page, MemoryError> {
        let addr = addr.align_down(PAGE_SIZE as u64);
        self.physical_address_space
            .get_prev(&addr)
            .map(|mapping| mapping.page_id(addr).map(|page_id| &self.pages[page_id.0]))
            .flatten()
            .ok_or(MemoryError::NoPhysicalMapping(addr))
    }

    pub fn page_mut(
        &mut self,
        addr: EPAddr,
    ) -> Result<&mut Page, MemoryError> {
        let addr = addr.align_down(PAGE_SIZE as u64);
        self.physical_address_space
            .get_prev(&addr)
            .map(|mapping| {
                mapping
                    .page_id(addr)
                    .map(|page_id| &mut self.pages[page_id.0])
            })
            .flatten()
            .ok_or(MemoryError::NoPhysicalMapping(addr))
    }

    pub fn page_by_id(
        &self,
        id: PageId,
    ) -> Result<&Page, MemoryError> {
        self.pages
            .get(id.0)
            .ok_or(MemoryError::InvalidPageId(Box::new(id)))
    }

    pub fn page_by_id_mut(
        &mut self,
        id: PageId,
    ) -> Result<&mut Page, MemoryError> {
        self.pages
            .get_mut(id.0)
            .ok_or(MemoryError::InvalidPageId(Box::new(id)))
    }

    pub fn page_id(
        &self,
        addr: EPAddr,
    ) -> Result<PageId, MemoryError> {
        let addr = addr.align_down(PAGE_SIZE as u64);
        self.physical_address_space
            .get_prev(&addr)
            .map(|mapping| mapping.page_id(addr))
            .flatten()
            .ok_or(MemoryError::NoPhysicalMapping(addr))
    }
}

#[cfg(test)]
mod test {
    use crate::physical::PhysicalMemory;
    use crate::PAGE_SIZE;
    use mogi_system::components::mmu::Permission;
    use mogi_utilities::addresses::EPAddr;

    #[test]
    fn test() {
        let mut mem = PhysicalMemory::new(5);
        let result = mem
            .alloc(EPAddr::from(0x8000_u64), 1_000_000 * PAGE_SIZE, Permission::READ | Permission::WRITE)
            .unwrap();
        drop(result);
        mem.free(0x9000_u64.into(), 10_000 * PAGE_SIZE).unwrap();
        let page_data = mem.page_mut(0x8000_u64.into()).unwrap().data_mut();
        for i in 0..PAGE_SIZE {
            page_data[i] = 0xff
        }
        let result = mem.page(EPAddr::from(0x8500u64));
        println!("{:?}", result);
    }
}
