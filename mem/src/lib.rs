use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};

pub mod mmu;
pub mod page_table;
pub mod physical;
pub mod tlb;

/// The number of bits required to represent an offset in a page from the page base address
pub const PAGE_OFFSET_BITS: usize = 12;

/// The size in bytes of a page
pub const PAGE_SIZE: usize = 1 << PAGE_OFFSET_BITS;

pub const MAX_PAGE_OFFSET: usize = (1 << PAGE_OFFSET_BITS) - 1;

pub trait PageOffset {
    /// Calculate the offset from the start of the page where this address lives.
    fn page_offset(&self) -> usize;
}

impl<T> PageOffset for EVAddr<T> {
    fn page_offset(&self) -> usize {
        (*self & ((1 << PAGE_OFFSET_BITS) - 1)) as usize
    }
}

impl<T> PageOffset for EPAddr<T> {
    fn page_offset(&self) -> usize {
        (*self & ((1 << PAGE_OFFSET_BITS) - 1)) as usize
    }
}
