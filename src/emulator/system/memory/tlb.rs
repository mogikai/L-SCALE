use std::collections::BTreeMap;
use im_rc::{OrdMap, update_in};

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

pub struct TLB {
    mappings: OrdMap<EVAddr, TLBEntry>,
    snapshots: Vec<OrdMap<EVAddr, TLBEntry>>,
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

    pub fn try_insert(
        &mut self,
        address: EVAddr,
        entry: TLBEntry,
    ) -> Result<(), MemoryError> {
        let (a, e) = match self.mappings.get_prev(&address) {
            Some(e) => e,
            None => return Err(MemoryError::Unknown),
        };
        if (a + e.size) > address {
            return Err(MemoryError::Unknown);
        }
        update_in!(self.mappings, address, entry);
        return Ok(());
    }

    pub fn heap_end(&self) -> Result<EVAddr, MemoryError> {
        Err(MemoryError::Unknown)
    }
}

impl Restorable for TLB {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        let s = self.mappings.clone();
        self.mappings = s.clone();
        self.snapshots.push(s);
        return Ok(());
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.restore();
        self.snapshots.pop().ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot availible"))?;
        return Ok(());
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.mappings = self.snapshots.last().ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot availible"))?.clone();
        return Ok(());
    }
}
