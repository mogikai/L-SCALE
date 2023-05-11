use std::collections::BTreeMap;

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;

pub struct TLBEntry {
    pub physical_address: EPAddr,
    pub permissions: Permission,
}

pub struct TLB {
    mappings: BTreeMap<EVAddr, TLBEntry>,
}

impl TLB {
    pub fn new() -> TLB {
        TLB {
            mappings: Default::default(),
        }
    }

    pub fn translate_virtual_address(
        &self,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        todo!()
    }

    pub fn heap_end(&self) -> Result<EVAddr, MemoryError> {
        Err(MemoryError::Unknown)
    }
}

impl Restorable for TLB {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
