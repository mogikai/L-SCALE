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

    /// Checks whether the tlb contains an EVAddr mapping that contains the entire region
    /// from address to address + size
    fn mapped(
        &self,
        address: EVAddr,
        size: usize,
    ) -> bool {
        // Block that starts at address less than or equal to address
        let (evaddr, entry) = match self.mappings.get_prev(&address) {
            Some(e) => e,
            None => return false,
        };

        if (evaddr + entry.size) > address {
            return true;
        }

        let a = match self.mappings.get_next(&(evaddr + 1)) {
            Some((a, _)) => a,
            None => return false,
        };

        *a < (address + size)
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
        Ok(())
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
            .and_then(|(vaddress, entry)| {
                // Address is contained in block
                if vaddress + entry.size <= address {
                    Some((*vaddress, *entry))
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
