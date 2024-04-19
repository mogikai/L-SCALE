use crate::physical::PageId;
use crate::PAGE_SIZE;
use addr_align::Align;
use mogi_utilities::addresses::EPAddr;
use mogi_utilities::error::memory::MemoryError;
use std::cmp::{
    max,
    min,
    Ordering,
};
use std::fmt::{
    Display,
    Formatter,
};
use std::ops::Range;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct PhysicalMapping {
    pub start: EPAddr,
    pub size: usize,
    pub page_ids: Rc<imbl::Vector<PageId>>,
}

impl Display for PhysicalMapping {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        let size = format!("({:#x})", self.size);
        write!(f, "{:?}..{:?} {:>10} -> {:?}", self.start, self.end(), size, self.page_ids)
    }
}

impl PhysicalMapping {
    pub fn new(
        start: EPAddr,
        size: usize,
        page_ids: imbl::Vector<PageId>,
    ) -> Self {
        Self {
            start,
            size,
            page_ids: Rc::new(page_ids),
        }
    }

    pub fn end(&self) -> EPAddr {
        self.start + self.size
    }

    /// Get the page id corresponding to the provided address.
    /// If the address is not contained in the mapping, None is returned.
    pub fn page_id(
        &self,
        addr: EPAddr,
    ) -> Option<PageId> {
        if addr < self.start || addr > self.end() {
            None
        } else {
            let offset = (addr - self.start) / PAGE_SIZE;
            Some(self.page_ids[offset])
        }
    }

    pub fn cutout(
        mut self,
        cutout_address: EPAddr,
        cutout_size: usize,
    ) -> Result<(Option<PhysicalMapping>, PhysicalMapping, Option<PhysicalMapping>), MemoryError> {
        if cutout_address > self.end()
            || !cutout_address.is_aligned(PAGE_SIZE as u64)
            || !cutout_size.is_aligned(PAGE_SIZE)
        {
            Err(MemoryError::InvalidArguments)?
        }
        let cutout_start = max(cutout_address, self.start);
        let cutout_end = min(cutout_address + cutout_size, self.end());
        let cutout_size = cutout_end - cutout_start;
        assert!(cutout_size.is_aligned(PAGE_SIZE));

        let offset = cutout_start - self.start;
        let page_ids_remainder = Rc::make_mut(&mut self.page_ids).split_off(offset / PAGE_SIZE);
        let front = offset.gt(&0).then_some(PhysicalMapping {
            start: self.start,
            size: offset,
            page_ids: self.page_ids,
        });

        let mut middle_page_ids = page_ids_remainder;
        let back_page_ids = middle_page_ids.split_off(cutout_size / PAGE_SIZE);

        let middle = PhysicalMapping {
            start: self.start + offset,
            size: cutout_size,
            page_ids: Rc::new(middle_page_ids),
        };

        let back = (self.start + offset + cutout_size)
            .lt(&(self.start + self.size))
            .then_some(PhysicalMapping {
                start: self.start + offset + cutout_size,
                size: self.size - offset - cutout_size,
                page_ids: Rc::new(back_page_ids),
            });

        Ok((front, middle, back))
    }

    pub fn overlaps(
        &self,
        range: Range<EPAddr>,
    ) -> bool {
        if self.end() <= range.start {
            false
        } else {
            range.end > self.start
        }
    }
}

impl PartialEq for PhysicalMapping {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.start.eq(&other.start)
    }
}

impl Eq for PhysicalMapping {}

impl PartialOrd for PhysicalMapping {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PhysicalMapping {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl std::borrow::Borrow<EPAddr> for PhysicalMapping {
    fn borrow(&self) -> &EPAddr {
        &self.start
    }
}

pub trait Contains<T> {
    fn contains(
        &self,
        val: T,
    ) -> bool;
}

impl Contains<EPAddr> for PhysicalMapping {
    fn contains(
        &self,
        address: EPAddr,
    ) -> bool {
        address >= self.start && address < self.start + self.size
    }
}

impl Contains<&Range<EPAddr>> for PhysicalMapping {
    fn contains(
        &self,
        range: &Range<EPAddr>,
    ) -> bool {
        self.contains(range.start) && self.contains(range.end)
    }
}
