use std::cmp::Ordering;
use std::ops::Range;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

#[derive(Copy, Clone)]
pub struct Region {
    pub v_start: EVAddr,
    pub p_start: EPAddr,
    pub size: usize,
    pub permissions: Permission,
}

impl Region {
    pub fn make_tlb_entry(
        &self,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        Ok(TLBEntry::new(address, self.p_start + (address - self.v_start), self.permissions))
    }
}

impl PartialEq for Region {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.v_start.eq(&other.v_start)
    }
}

impl Eq for Region {}

impl PartialOrd for Region {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        self.v_start.partial_cmp(&other.v_start)
    }
}

impl Ord for Region {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.v_start.cmp(&other.v_start)
    }
}

impl std::borrow::Borrow<EVAddr> for Region {
    fn borrow(&self) -> &EVAddr {
        &self.v_start
    }
}

pub trait Contains<T> {
    fn contains(
        &self,
        val: T,
    ) -> bool;
}

impl Contains<EVAddr> for Region {
    fn contains(
        &self,
        address: EVAddr,
    ) -> bool {
        address >= self.v_start && address < self.v_start + self.size
    }
}

impl Contains<&Range<EVAddr>> for Region {
    fn contains(
        &self,
        vadd_range: &Range<EVAddr>,
    ) -> bool {
        self.contains(vadd_range.start) && self.contains(vadd_range.end)
    }
}
