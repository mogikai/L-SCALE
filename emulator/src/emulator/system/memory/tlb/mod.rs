use std::cmp::{max, min};
use std::collections::btree_set::Iter;
use std::collections::BTreeSet;
use std::ops::{BitOr, Not};

use im_rc::Vector;
use itertools::{Itertools, PeekingNext};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::mmu::{MemoryError, Permission};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::memory::helpers::is4kaligned;
use crate::emulator::system::memory::pregion::PhysicalRegion;
use crate::emulator::system::memory::tlb::mapping::{Contains, Mapping, MappingType};
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

pub mod mapping;
mod tests;
pub mod tlb_entry;

#[derive(Clone, Debug)]
pub struct TLB {
    mappings: BTreeSet<Mapping>,
    snapshots: Vector<BTreeSet<Mapping>>,
}

impl Default for TLB {
    fn default() -> Self {
        TLB::new()
    }
}

impl TLB {
    pub fn new() -> TLB {
        TLB {
            mappings: Default::default(),
            snapshots: Default::default(),
        }
    }

    pub fn mapping_iter(
        &self,
    ) -> Iter<Mapping> {
        self.mappings.iter()
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
        mapping_type: MappingType,
    ) -> Result<(), MemoryError> {
        if !is4kaligned(v_start) || !is4kaligned(p_start) || !is4kaligned(size) {
            Err(MemoryError::InvalidAlignment)?
        }
        self.region_mapped(v_start, size)
            .not()
            .then(|| {
                let inserted = self
                    .mappings
                    .insert(Mapping::new(v_start, p_start, size, permissions, mapping_type));
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

    /// Translates the virtual address range address..address+size and returns
    /// a list of TLBEntry objects this range corresponds to. The returned
    /// physical regions are ordered in ascending order from lowest to highest.
    pub fn translate_virtual_addresses(
        &self,
        address: EVAddr,
        size: usize,
    ) -> Result<Vec<TLBEntry>, MemoryError> {
        let mappings = self.get_mappings_copied(address, address + size)?;
        if !is_continuous(&mappings) {
            Err(MemoryError::NotContinuous)?
        }
        let mappings = mappings.into_iter().rev();

        let mut current_address = address;
        let mut current_size = size;
        let mut physical_regions = Vec::with_capacity(mappings.len());
        for mapping in mappings {
            let mut tlb_entry = mapping.make_tlb_entry(current_address)?;
            if current_size < tlb_entry.size {
                tlb_entry.size = current_size;
            }
            current_size -= tlb_entry.size;
            current_address = tlb_entry.virtual_address + tlb_entry.size;
            physical_regions.push(tlb_entry)
        }
        Ok(physical_regions)
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

    /// Get a copy of all mappings that are contained in the specified range start..end
    /// The mappings are returned in the order highest address to lowest address
    pub fn get_mappings_copied(
        &self,
        start: EVAddr,
        end: EVAddr,
    ) -> Result<Vec<Mapping>, MemoryError> {
        Ok(self
            .mappings
            .range(..end)
            .rev()
            .take_while(|m| start < m.v_end())
            .copied()
            .collect())
    }

    pub fn protect(
        &mut self,
        start: EVAddr,
        size: usize,
        permissions: Permission,
    ) -> Result<(), MemoryError> {
        let mappings = self.get_mappings_copied(start, start + size)?;
        if !is_continuous(&mappings) {
            Err(MemoryError::NotContinuous)?
        }

        for mapping in mappings.iter().rev() {
            let vaddr = mapping.v_start;
            let mapping = self.mappings.take(&vaddr).unwrap();
            let (front, mut middle, back) = mapping.cutout(start, size)?;
            middle.permissions = permissions;
            front.map(|r| self.mappings.insert(r));
            back.map(|r| self.mappings.insert(r));
            self.mappings.insert(middle);
        }
        Ok(())
    }

    /// Cuts out the region start..end from the mapped regions and returns the freed physical region(s)
    /// Any unmapped parts of the address region are simply ignored. unmap on a region where no mappings
    /// exist is a noop
    pub fn unmap(
        &mut self,
        start: EVAddr,
        size: usize,
    ) -> Result<Vec<PhysicalRegion>, MemoryError> {
        Ok(self
            .get_mappings_copied(start, start + size)?
            .into_iter()
            .rev()
            .filter_map(|m| {
                // Remove the original mapping so we can modify it
                let mapping = self
                    .mappings
                    .take(&m.v_start)
                    .expect("The mapping must exist since the addresses were previously gathered from the collection");

                // Clamp cutout to mapping boundaries with max and min
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
        self.mappings.remove(&mapping)
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

/// Takes a reversed list of mappings (the list is ordered highest to lowest start address)
/// and checks if the mappings are continuous
fn is_continuous(mappings: &[Mapping]) -> bool {
    if mappings.is_empty() {
        return true;
    }

    let mut prev_mapping = mappings.last().unwrap();
    for mapping in mappings.iter().rev().skip(1) {
        if prev_mapping.v_end() != mapping.v_start {
            return false;
        }
        prev_mapping = mapping;
    }
    true
}
