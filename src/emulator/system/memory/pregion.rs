use std::cmp::{min, Ordering};

use crate::emulator::abstractions::cpu::address::EPAddr;

#[derive(Debug, Copy, Clone)]
pub struct PhysicalRegion {
    pub start: EPAddr,
    pub end: EPAddr,
}

impl PhysicalRegion {
    pub fn new(
        start: EPAddr,
        end: EPAddr,
    ) -> Self {
        Self { start, end }
    }

    pub fn contains(
        &self,
        address: EPAddr,
    ) -> bool {
        self.start <= address && address < self.end
    }

    pub fn overlaps(
        &self,
        other: &PhysicalRegion,
    ) -> bool {
        if self.end <= other.start {
            false
        } else if other.end <= self.start {
            false
        } else {
            true
        }
    }

    pub fn split(
        self,
        size: usize,
    ) -> (Option<PhysicalRegion>, Option<PhysicalRegion>) {
        let front = size.gt(&0).then(|| PhysicalRegion {
            start: self.start,
            end: self.start + min(size, self.size()),
        });

        let back = size.lt(&self.size()).then(|| PhysicalRegion {
            start: self.start + size,
            end: self.end,
        });

        (front, back)
    }

    pub fn size(&self) -> usize {
        self.end - self.start
    }
}

impl PartialEq for PhysicalRegion {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.start.eq(&other.start)
    }
}

impl Eq for PhysicalRegion {}

impl PartialOrd for PhysicalRegion {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

impl Ord for PhysicalRegion {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl std::borrow::Borrow<EPAddr> for PhysicalRegion {
    fn borrow(&self) -> &EPAddr {
        &self.start
    }
}
