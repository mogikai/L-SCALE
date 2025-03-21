use crate::emulator::system::fd_map::InternalFD;
use mogi_system::components::mmu::Permission;
use mogi_system::components::mmu::tlb_entry::TLBEntry;
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use mogi_utilities::overlap::overlaps;
use std::cmp::{
    Ordering,
    max,
    min,
};
use std::fmt::{
    Display,
    Formatter,
};
use std::ops::Range;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MappingType {
    /// Private mapping
    PrivateAnonymous,
    /// Private file backed
    PrivateFile(InternalFD),
    /// Shared but not file backed
    SharedAnonymous,
    /// Shared File backed
    SharedFile(InternalFD),
}

#[derive(Clone, Debug)]
pub struct Mapping {
    pub v_start: EVAddr,
    pub p_start: Option<EPAddr>,
    pub size: usize,
    pub permissions: Permission,
    pub guard_callback_id: Option<usize>,
    pub mapping_type: MappingType,
    pub label: Option<String>,
}

impl Display for Mapping {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        let label = if let Some(label) = &self.label {
            format!(" [{}] ", label)
        } else {
            "".to_string()
        };
        let physical = if let Some(p_start) = self.p_start {
            format!("{:?}..{:?}", p_start, self.p_end().unwrap())
        } else {
            "[nil]".to_string()
        };
        let perm = format!("{:?}", self.permissions);
        let size = format!("({:#x})", self.size);
        write!(f, "{:?}..{:?} -> {} {:>10} {:<30}{}", self.v_start, self.v_end(), physical, size, perm, label)
    }
}

impl Mapping {
    pub fn new(
        v_start: EVAddr,
        p_start: EPAddr,
        size: usize,
        permissions: Permission,
        mapping_type: MappingType,
        label: Option<String>,
    ) -> Self {
        Self {
            v_start,
            p_start: Some(p_start),
            size,
            permissions,
            guard_callback_id: None,
            mapping_type,
            label,
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
            mapping_type: MappingType::PrivateAnonymous,
            label: Some("Guard".to_string()),
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
        let result = TLBEntry::new(
            address,
            p_start + (address - self.v_start),
            self.permissions,
            self.size - (address - self.v_start),
        );
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
        if cutout_address > self.v_end() {
            Err(MemoryError::InvalidArguments)?
        }
        let cutout_start = max(cutout_address, self.v_start);
        let cutout_end = min(cutout_address + cutout_size, self.v_end());
        let cutout_size = cutout_end - cutout_start;

        let offset = cutout_start - self.v_start;

        let front = offset.gt(&0).then_some(Mapping {
            v_start: self.v_start,
            size: offset,
            permissions: self.permissions,
            p_start: self.p_start,
            guard_callback_id: self.guard_callback_id,
            mapping_type: self.mapping_type,
            label: self.label.clone(),
        });

        let middle = Mapping {
            v_start: self.v_start + offset,
            size: cutout_size,
            permissions: self.permissions,
            p_start: self.p_start.map(|pa| pa + offset),
            guard_callback_id: self.guard_callback_id,
            mapping_type: self.mapping_type,
            label: self.label.clone(),
        };

        let back = (self.v_start + offset + cutout_size)
            .lt(&self.v_end())
            .then_some(Mapping {
                v_start: self.v_start + offset + cutout_size,
                size: self.size - offset - cutout_size,
                permissions: self.permissions,
                p_start: self.p_start.map(|pa| pa + offset + cutout_size),
                guard_callback_id: self.guard_callback_id,
                mapping_type: self.mapping_type,
                label: self.label.clone(),
            });

        Ok((front, middle, back))
    }

    pub fn overlaps(
        &self,
        range: Range<EVAddr>,
    ) -> bool {
        overlaps(self.v_start..self.v_end(), range)
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
        Some(self.cmp(other))
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
