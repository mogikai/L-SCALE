use std::cmp::{max, min};
use std::collections::BTreeSet;
use std::ops::{BitOr, Not, Range};

use im_rc::{OrdSet, Vector};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::is4kaligned;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::tlb::mapping::{Contains, Mapping};
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

pub mod mapping;
pub mod tlb_entry;

#[derive(Clone, Debug)]
pub struct TLB {
    mappings: BTreeSet<Mapping>,
    snapshots: Vector<BTreeSet<Mapping>>,
}

impl TLB {
    pub fn new() -> TLB {
        TLB {
            mappings: Default::default(),
            snapshots: Default::default(),
        }
    }

    pub fn address_mapped(
        &self,
        address: EVAddr,
    ) -> bool {
        self.mappings
            .range(..=address)
            .next_back()
            .filter(|mapping| mapping.contains(address))
            .is_some()
    }

    /// Checks whether the tlb contains one or more EVAddr mappings
    /// that contain parts of the region from address to address + size.
    /// Returns true if any part of the region is mapped, and false otherwise.
    pub fn region_mapped(
        &self,
        address: EVAddr,
        size: usize,
    ) -> bool {
        // The first check only finds regions which start inside the range `address..address + size`.
        // To find regions that start outside this range, but still overlap, we have to check if the start or end
        // of our region is mapped somewhere.
        self.mappings
            .range(address..address + size)
            .next()
            .is_some()
            || self.address_mapped(address)
            || self.address_mapped(address + size - 1)
    }

    /// Maps the virtual memory area `v_start..(v_start + size)`
    /// to the physical memory area `p_start..(p_start + size)`
    /// with permissions `permissions`
    ///
    /// Results in an error if any part of the area `v_start..(v_start + size)` is already mapped.
    pub fn try_map(
        &mut self,
        v_start: EVAddr,
        p_start: EPAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        if !is4kaligned(v_start) || !is4kaligned(p_start) || !is4kaligned(size) {
            Err(MemoryError::InvalidAlignment)?
        }
        self.region_mapped(v_start, size)
            .not()
            .then(|| {
                let inserted = self
                    .mappings
                    .insert(Mapping::new(v_start, p_start, size, permissions));
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
        self.get_mapping(address)
            .and_then(|region| region.make_tlb_entry(address))
    }

    /// Looks up the mapping containing a given virtual address.
    pub fn get_mapping(
        &self,
        address: EVAddr,
    ) -> Result<Mapping, MemoryError> {
        self.mappings
            // First block that starts at address less than or equal to address
            .range(..=address)
            .next_back()
            .and_then(|region| region.contains(address).then_some(*region))
            .ok_or(MemoryError::NotMapped)
    }

    /// Get a reference to all mappings that are contained in the specified range start..end
    /// The mappings are returned in the order highest address to lowest address
    pub fn get_mappings(
        &self,
        start: EVAddr,
        end: EVAddr,
    ) -> Result<Vec<&Mapping>, MemoryError> {
        Ok(self
            .mappings
            .range(..end)
            .rev()
            .take_while(|m| start < m.v_end())
            .collect())
    }

    /// Cuts out the region start..end from the mapped regions and returns the freed physical region(s)
    pub fn unmap(
        &mut self,
        start: EVAddr,
        size: usize,
    ) -> Result<Vec<PhysicalRegion>, MemoryError> {
        let mappings = self
            .get_mappings(start, start + size)?
            .into_iter()
            .map(|r| r.v_start)
            .collect::<Vec<_>>();

        Ok(mappings
            .iter()
            .filter_map(|vaddr| {
                let mapping = self
                    .mappings
                    .take(vaddr)
                    .expect("The mapping must exist since the addresses were previously gathered from the collection");
                let (front, middle, back) = mapping
                    .cutout(max(start, mapping.v_start), min(size, mapping.size))
                    .expect("This cutout operation should never fail if implemented correctly");
                // Reinsert remainders
                front.map(|m| self.mappings.insert(m));
                back.map(|m| self.mappings.insert(m));
                // Return free physical regions if applicable
                middle
                    .p_start
                    .map(|pa| PhysicalRegion::new(pa, pa + middle.size))
            })
            .collect::<Vec<_>>())
    }

    pub fn resize(
        &mut self,
        addr: EVAddr,
        new_size: usize,
    ) -> Result<(), MemoryError> {
        // TODO: What about the physical address space? We need to check if space is free and otherwise
        //       create a new mapping?
        let mut region = *self.mappings.get(&addr).ok_or(MemoryError::NotMapped)?;
        if new_size > region.size && self.region_mapped(addr + region.size, new_size - region.size) {
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

    pub fn setperms(
        &mut self,
        addr: EVAddr,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        // TODO: What about the physical address space? We need to check if space is free and otherwise
        //       create a new mapping?
        let mut region = *self.mappings.get(&addr).ok_or(MemoryError::NotMapped)?;
        region.permissions = permissions;
        self.mappings
            .replace(region)
            .expect("This operation should replace an existing mapping");
        return Ok(());
    }

    /// Finds the next free virtual memory region in this process groups mapping starting at `hint`.
    pub fn find_free_space(
        &self,
        mut hint: EVAddr,
        size: usize,
    ) -> Result<EVAddr, MemoryError> {
        for mapping in self.mappings.range(hint..) {
            if mapping.contains(hint) || mapping.contains(hint + size - 1) {
                hint = mapping.v_end();
            } else {
                break;
            }
        }
        Ok(hint)
    }

    pub fn remove(
        &mut self,
        mapping: Mapping,
    ) -> bool {
        return self.mappings.remove(&mapping);
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
