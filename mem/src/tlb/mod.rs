pub mod entry;
pub mod tlb;

use crate::PAGE_OFFSET_BITS;
use mogi_utilities::addresses::EVAddr;

// TODO: Experiment with tlb sizes, and with per-process tables
pub const TLB_INDEX_BITS: usize = 12;
pub const TLB_SIZE: usize = 1 << TLB_INDEX_BITS;
pub const TLB_TAG_BITS: usize = EVAddr::<u8>::bits() - (crate::PAGE_OFFSET_BITS + TLB_INDEX_BITS);

pub trait PageIndex {
    fn index(&self) -> u64;

    /// The number of bits to shift to get the index
    fn index_shift_from_start() -> u8;

    /// The mask to apply to the shifted value to extract the actual index.
    fn index_mask() -> u64;
}

pub trait PageTag {
    fn tag(&self) -> u64;

    /// The number of bits to shift from the index to get the tag.
    fn tag_shift_from_index() -> u8;

    /// The mask to apply to the shifted value to extract the actual tag.
    fn tag_mask() -> u64;
}

impl<T> PageIndex for EVAddr<T> {
    fn index(&self) -> u64 {
        (self >> PAGE_OFFSET_BITS) & ((1 << TLB_INDEX_BITS) - 1)
    }

    fn index_shift_from_start() -> u8 {
        PAGE_OFFSET_BITS
            .try_into()
            .expect("Shift bits must fit in a u8")
    }

    fn index_mask() -> u64 {
        (1 << TLB_INDEX_BITS) - 1
    }
}

impl<T> PageTag for EVAddr<T> {
    fn tag(&self) -> u64 {
        (self >> (PAGE_OFFSET_BITS + TLB_INDEX_BITS)) & ((1 << TLB_TAG_BITS) - 1)
    }

    fn tag_shift_from_index() -> u8 {
        TLB_INDEX_BITS
            .try_into()
            .expect("Shift bits must fit in a u8")
    }

    fn tag_mask() -> u64 {
        (1 << TLB_TAG_BITS) - 1
    }
}

#[cfg(test)]
mod test {
    use crate::physical::Page;
    use crate::tlb::tlb::TLB;
    use mogi_utilities::addresses::{
        EVAddr,
        HVAddr,
    };

    #[test]
    fn test() {
        let mut tlb = TLB::new();
        let addr = EVAddr::from(0xc0_00a_f14_u64);
        println!("{:?}", unsafe { tlb.page(addr) });
        let mut page = Page::new();
        let old_page = page.clone();
        page.data_mut()[5] = 5;
        unsafe {
            //tlb.insert_read(addr, HVAddr::new_unchecked(&mut page as *mut _));
        }
        println!("{:?}", unsafe { tlb.page(addr) });
        println!("{:?}", unsafe { old_page });
    }
}
