pub mod entry;
pub mod tlb;

use crate::PAGE_OFFSET_BITS;
use mogi_utilities::addresses::EVAddr;
use std::convert::Into;

// TODO: Experiment with tlb sizes, and with per-process tables
pub const TLB_INDEX_BITS: usize = 12;
pub const TLB_SIZE: usize = 1 << TLB_INDEX_BITS;
pub const TLB_TAG_BITS: usize = EVAddr::<u8>::bits() - (crate::PAGE_OFFSET_BITS + TLB_INDEX_BITS);

pub trait PageIndex {
    fn index(&self) -> u64;
}

pub trait PageTag {
    fn tag(&self) -> u64;
}

impl<T> PageIndex for EVAddr<T> {
    fn index(&self) -> u64 {
        (self >> PAGE_OFFSET_BITS) & ((1 << TLB_INDEX_BITS) - 1)
    }
}

impl<T> PageTag for EVAddr<T> {
    fn tag(&self) -> u64 {
        (self >> (PAGE_OFFSET_BITS + TLB_INDEX_BITS)) & ((1 << TLB_TAG_BITS) - 1)
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
            tlb.insert_read(addr, HVAddr::new_unchecked(&mut page as *mut _));
        }
        println!("{:?}", unsafe { tlb.page(addr) });
        println!("{:?}", unsafe { old_page });
    }
}
