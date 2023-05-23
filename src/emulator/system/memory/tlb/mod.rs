use std::collections::BTreeSet;
use std::ops::{Not, Range};

use im_rc::{OrdSet, Vector};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::tlb::region::{Contains, Region};
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

pub mod region;
pub mod tlb_entry;

#[derive(Clone)]
pub struct TLB {
    mappings: BTreeSet<Region>,
    snapshots: Vector<BTreeSet<Region>>,
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
            .range(address..(address + size))
            .next()
            .is_some()
    }

    pub fn try_insert(
        &mut self,
        region: Region,
    ) -> Result<(), MemoryError> {
        self.mapped(region.v_start, region.size)
            .not()
            .then(|| {
                let inserted = self.mappings.insert(region);
                assert!(inserted)
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
        self.get_region(address)
            .and_then(|region| region.make_tlb_entry(address))
    }

    /// Looks up the region containing a given virtual address.
    pub fn get_region(
        &self,
        address: EVAddr,
    ) -> Result<Region, MemoryError> {
        self.mappings
            // First block that starts at address less than or equal to address
            .range(..=address)
            .next_back()
            .and_then(|region| region.contains(address).then_some(*region))
            .ok_or(MemoryError::NotMapped)
    }

    pub fn resize(
        &mut self,
        addr: EVAddr,
        new_size: usize,
    ) -> Result<(), MemoryError> {
        // TODO: What about the physical address space? We need to check if space is free and otherwise
        //       create a new mapping?
        let mut region = *self.mappings.get(&addr).ok_or(MemoryError::NotMapped)?;
        if new_size > region.size && self.mapped(addr + region.size, new_size - region.size) {
            // Cannot extend the region since the address space after the region is already mapped
            Err(MemoryError::AlreadyMapped)
        } else {
            region.size = new_size;
            self.mappings
                .replace(region)
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
