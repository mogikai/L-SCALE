use std::ops::Not;

use im_rc::{OrdMap, Vector};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;

#[derive(Copy, Clone)]
pub struct TLBEntry {
    pub physical_address: EPAddr,
    pub size: usize,
    pub permissions: Permission,
}

impl TLBEntry {
    pub fn new(
        physical_address: EPAddr,
        size: usize,
        permissions: Permission,
    ) -> Self {
        Self {
            physical_address,
            size,
            permissions,
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

    /// Checks whether the tlb contains one or more EVAddr mappings
    /// that contain parts of the region from address to address + size.
    /// Returns true if any part of the region is mapped, and false otherwise.
    fn mapped(
        &self,
        address: EVAddr,
        size: usize,
    ) -> bool {
        self.mappings
            .range(&address..&(address + size))
            .next()
            .is_some()
    }

    pub fn try_insert(
        &mut self,
        address: EVAddr,
        entry: TLBEntry,
    ) -> Result<(), MemoryError> {
        self.mapped(address, entry.size)
            .not()
            .then(|| {
                let old = self.mappings.insert(address, entry);
                assert!(old.is_none())
            })
            .ok_or(MemoryError::AlreadyMapped)
    }

    /// Translates the given virtual address `address` into a physical address.
    /// The returned [`TLBEntry`] contains the physical address and the permissions
    /// that are set on the memory region.
    pub fn translate_virtual_address(
        &self,
        address: EVAddr,
    ) -> Result<TLBEntry, MemoryError> {
        self.get_mapping(address).map(|(_, tlb_entry)| tlb_entry)
    }

    /// Looks up the tlb entry for a given address.
    /// Returns the entry and the start virtual address to which the entry is mapped.
    pub fn get_mapping(
        &self,
        address: EVAddr,
    ) -> Result<(EVAddr, TLBEntry), MemoryError> {
        self.mappings
            // First block that starts at address less than or equal to address
            .get_prev(&address)
            .and_then(|(block_start, entry)| {
                // Address is contained in block
                if address < block_start + entry.size {
                    Some((*block_start, *entry))
                } else {
                    None
                }
            })
            .ok_or(MemoryError::NotMapped)
    }

    pub fn realloc(
        &mut self,
        addr: EVAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        let mut tlb_entry = *self.mappings.get(&addr).ok_or(MemoryError::Unknown)?;
        if tlb_entry.size >= size {
            tlb_entry.size = size;
            self.mappings
                .insert(addr, tlb_entry)
                .expect("This operation should replace an existing mapping");
            Ok(())
        } else if self.mapped(addr + tlb_entry.size, size - tlb_entry.size) {
            Err(MemoryError::Unknown)
        } else {
            tlb_entry.size = size;
            self.mappings
                .insert(addr, tlb_entry)
                .expect("This operation should replace an existing mapping");
            Ok(())
        }
    }
}

impl Restorable for TLB {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push_back(self.mappings.clone());
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.restore()?;
        self.snapshots
            .pop_back()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot available"))?;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.mappings = self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot available"))?
            .clone();
        Ok(())
    }
}
