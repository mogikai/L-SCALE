use std::collections::BTreeMap;
use im_rc::{OrdMap, Vector};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;

#[derive(Copy, Clone)]
pub struct TLBEntry {
    pub physical_address: EPAddr,
    pub size: u64,
    pub permissions: Permission,
}

impl TLBEntry {
    pub fn new(paddr: EPAddr, size: u64, perms: Permission) -> Self {
        Self {
            physical_address: paddr,
            size: size,
            permissions: perms,
        }
    }
}

#[derive(Clone)]
pub struct TLB {
    mappings: OrdMap<EVAddr, TLBEntry>,
    snapshots: Vector<OrdMap<EVAddr, TLBEntry>>,
}

impl TLB {
    pub fn new() -> TLB {
        TLB {
            mappings: Default::default(),
            snapshots: Default::default(),
        }
    }

    pub fn translate_virtual_address(
        &self,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        let (a, e) = match self.mappings.get_prev(&address) {
            Some(e) => e,
            None => return Err(MemoryError::Unknown),
        };
        if (a + e.size) <= address {
            return Err(MemoryError::Unknown);
        }
        return Ok(*e);
    }

    fn mapped(
        &self,
        address: EVAddr,
        size: u64
    ) -> bool {
        let (a, e) = match self.mappings.get_prev(&address) {
            Some(e) => e,
            None => return false,
        };

        if (a + e.size) > address {
            return true;
        }

        let (a, e) = match self.mappings.get_next(&(a + 1)) {
            Some(e) => e,
            None => return false,
        };

        return *a < (address + size);
    }

    pub fn try_insert(
        &mut self,
        address: EVAddr,
        entry: TLBEntry,
    ) -> Result<(), MemoryError> {
        if self.mapped(address, entry.size) {
            return Err(MemoryError::Unknown);
        }
        self.mappings.insert(address, entry);
        return Ok(());
    }

    pub fn get_mapping(
        &self,
        address: EVAddr,
    ) -> Result<(EVAddr, TLBEntry), MemoryError> {
        let (a, e) = self.mappings.get_prev(&address).ok_or(MemoryError::Unknown)?;
        if (a + e.size) <= address {
            return Err(MemoryError::Unknown);
        }
        return Ok((*a, *e));
    }

    pub fn realloc(
        &mut self,
        addr: EVAddr,
        size: u64
    ) -> Result<(), MemoryError> {
        let e = self.mappings.get(&addr).ok_or(MemoryError::Unknown)?;
        let mut e = *e;
        if e.size >= size {
            e.size = size;
            self.mappings.insert(addr, e);
            return Ok(());
        }
        if self.mapped(addr + e.size, size - e.size) {
            return Err(MemoryError::Unknown);
        }
        e.size = size;
        self.mappings.insert(addr, e);
        return Ok(());
    }

}

impl Restorable for TLB {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let s = self.mappings.clone();
        self.mappings = s.clone();
        self.snapshots.push_back(s);
        return Ok(());
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.restore();
        self.snapshots.pop_back().ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot availible"))?;
        return Ok(());
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.mappings = self.snapshots.last().ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot availible"))?.clone();
        return Ok(());
    }
}
