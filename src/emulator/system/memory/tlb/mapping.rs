use std::cmp::{max, min, Ordering};
use std::ops::Range;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

#[derive(Copy, Clone, Debug)]
pub struct Mapping {
    pub v_start: EVAddr,
    pub p_start: Option<EPAddr>,
    pub size: usize,
    pub permissions: Permission,
    pub guard_callback_id: Option<usize>,
}

impl Mapping {
    pub fn new(
        v_start: EVAddr,
        p_start: EPAddr,
        size: usize,
        permissions: Permission,
    ) -> Self {
        Self {
            v_start,
            p_start: Some(p_start),
            size,
            permissions,
            guard_callback_id: None,
        }
    }

    // TODO: Does this make sense? Should we also add some physical areas with canary values to maybe detect buffer overwrites?
    pub fn new_guard(
        v_start: EVAddr,
        size: usize,
        guard_callback_id: usize,
    ) -> Self {
        Self {
            v_start,
            p_start: None,
            size,
            permissions: Permission::NONE,
            guard_callback_id: Some(guard_callback_id),
        }
    }

    pub fn make_tlb_entry(
        &self,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        let p_start = self.p_start.ok_or_else(|| MemoryError::GuardRegionAccess {
            address,
            guard_callback_id: self
                .guard_callback_id
                .expect("If p_start is None then a guard_callback_id must exist"),
        })?;
        let result = TLBEntry::new(address, p_start + (address - self.v_start), self.permissions);
        Ok(result)
    }

    /// Returns the end virtual memory address of this mapping
    pub fn v_end(&self) -> EVAddr {
        self.v_start + self.size
    }

    /// Returns the end physical memory address of this mapping if it has one.
    pub fn p_end(&self) -> Option<EPAddr> {
        self.p_start.map(|pa| pa + self.size)
    }

    pub fn cutout(
        self,
        cutout_address: EVAddr,
        cutout_size: usize,
    ) -> Result<(Option<Mapping>, Mapping, Option<Mapping>), MemoryError> {
        if !self.contains(cutout_address) || cutout_address + cutout_size > self.v_end() {
            Err(MemoryError::Unknown)?
        }

        let offset = cutout_address - self.v_start;

        let front = offset.gt(&0).then_some(Mapping {
            v_start: self.v_start,
            size: offset,
            permissions: self.permissions,
            p_start: self.p_start,
            guard_callback_id: self.guard_callback_id,
        });

        let middle = Mapping {
            v_start: self.v_start + offset,
            size: cutout_size,
            permissions: self.permissions,
            p_start: self.p_start.map(|pa| pa + offset),
            guard_callback_id: self.guard_callback_id,
        };

        let back = (self.v_start + offset + cutout_size)
            .lt(&self.v_end())
            .then_some(Mapping {
                v_start: self.v_start + offset + cutout_size,
                size: self.size - offset - cutout_size,
                permissions: self.permissions,
                p_start: self.p_start.map(|pa| pa + offset + cutout_size),
                guard_callback_id: self.guard_callback_id,
            });

        Ok((front, middle, back))
    }
}

impl PartialEq for Mapping {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.v_start.eq(&other.v_start)
    }
}

impl Eq for Mapping {}

impl PartialOrd for Mapping {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        self.v_start.partial_cmp(&other.v_start)
    }
}

impl Ord for Mapping {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.v_start.cmp(&other.v_start)
    }
}

impl std::borrow::Borrow<EVAddr> for Mapping {
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

impl Contains<EVAddr> for Mapping {
    fn contains(
        &self,
        address: EVAddr,
    ) -> bool {
        address >= self.v_start && address < self.v_start + self.size
    }
}

impl Contains<&Range<EVAddr>> for Mapping {
    fn contains(
        &self,
        vadd_range: &Range<EVAddr>,
    ) -> bool {
        self.contains(vadd_range.start) && self.contains(vadd_range.end)
    }
}
