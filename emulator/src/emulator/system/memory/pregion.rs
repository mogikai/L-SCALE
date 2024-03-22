use std::cmp::{
    min,
    Ordering,
};
use std::fmt::{
    Debug,
    Formatter,
};

use addr_align::Align4k;
use mogi_utilities::addresses::EPAddr;
use size_format::SizeFormatterBinary;

#[derive(Copy, Clone)]
pub struct PhysicalRegion {
    pub start: EPAddr,
    pub end: EPAddr,
}

impl Debug for PhysicalRegion {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(
            f,
            "PhysicalRegion {{ start: {:?}, end: {:?}, size: {}B}}",
            self.start,
            self.end,
            SizeFormatterBinary::new(self.size() as u64)
        )
    }
}

impl PhysicalRegion {
    pub fn new(
        start: EPAddr,
        end: EPAddr,
    ) -> Self {
        assert!(start.is_4kaligned() && end.is_4kaligned());
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
        } else {
            other.end > self.start
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

    pub fn split_back(
        self,
        size: usize,
    ) -> (Option<PhysicalRegion>, Option<PhysicalRegion>) {
        self.split(self.size().saturating_sub(size))
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
        Some(self.cmp(other))
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
