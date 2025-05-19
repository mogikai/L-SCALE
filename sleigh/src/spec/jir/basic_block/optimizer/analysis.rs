use std::collections::hash_map::Entry;

use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use itertools::Itertools;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use mogi_utilities::{
    new_map_key,
    overlap,
};

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::TargetBasicBlockInner;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

#[derive(Clone, Debug, Default)]
pub(super) struct ReferencedSlices {
    pub(super) write: Option<LocalSliceId>,
    pub(super) reads: Vec<LocalSliceId>,
}

impl ReferencedSlices {
    fn new(
        write: Option<LocalSliceId>,
        reads: Vec<LocalSliceId>,
    ) -> Self {
        Self { write, reads }
    }
}

impl TargetBasicBlockInner {
    /// Store a unique id for each slice and annotate instructions with their
    /// input and output slices.
    pub(super) fn collect_slice_references(&mut self) -> ProcessorSpecResult {
        for inst in &self.jir_instructions {
            let rs = get_referenced_slices(
                inst,
                &mut self.optimizer_ancillary.slice_to_id,
                &mut self.optimizer_ancillary.slice_ids,
            );

            self.optimizer_ancillary.inst_slice_references.push(rs);
        }

        Ok(())
    }

    /// Store a list of overlapping slices for each slice.
    pub(super) fn collect_slice_overlaps(&mut self) -> ProcessorSpecResult {
        for (slice_id, slice) in self.optimizer_ancillary.slice_ids.items() {
            let mut overlap_group = FnvHashSet::default();

            for (neighbor_candidate_id, neighbor_candidate) in self.optimizer_ancillary.slice_ids.items() {
                // if they don't belong to the same group they can't overlap
                if slice.overlaps(neighbor_candidate) {
                    // overlap

                    // add neighbor to own overlap group group
                    overlap_group.insert(neighbor_candidate_id);
                }
            }

            // check if overlap groups of slice and any of the neighbors is the same and if so,
            // reuse
            let mut group_id = None;
            for neighbor_id in overlap_group.iter() {
                let Some(neighbor_group_id) = self.optimizer_ancillary.group_ids[*neighbor_id] else {
                    // neighbor might not have been processed yet
                    continue;
                };

                if self.optimizer_ancillary.overlap_groups[neighbor_group_id] == overlap_group {
                    // equal groups => reuse id
                    group_id = Some(neighbor_group_id);
                    break;
                }
            }

            let new_group_id = match group_id {
                Some(group_id) => group_id,
                None => self
                    .optimizer_ancillary
                    .overlap_groups
                    .insert(overlap_group),
            };

            self.optimizer_ancillary.group_ids[slice_id] = Some(new_group_id);
        }
        Ok(())
    }

    /// Populate `block_reads` and `block_writes` and  mark dead-store instructions.
    pub(super) fn calculate_block_reads_and_writes(&mut self) -> ProcessorSpecResult {
        // this is a two-phase algorithm:
        // 1. calculate reads and writes for each block locally
        // 2. use fixed-point iteration to solve cycles

        // step 1 (local blocks)
        for (block_id, block) in self.jir_blocks.items() {
            let Some(block) = block else {
                panic!("This must be Some");
            };

            let mut dead_stores = Vec::new();

            // iterate over the instructions in reverse, storing

            // In a local setting, we must assume all written slices are spilled.
            // We also store which instruction spilled them.
            let mut spilled_slices = SecondaryMap::<LocalSliceId, Option<InstId>>::default();

            // If a slice is written and has no read entry, we know it's a dead store.
            // We also store which instruction read it.
            let mut read_slices = SecondaryMap::<LocalSliceId, Option<InstId>>::default();

            for inst_id in block.range.clone().rev() {
                let inst = &self.jir_instructions[inst_id];
                let rs = &self.optimizer_ancillary.inst_slice_references[inst_id];

                // process written slices (mark spills and check if current inst is dead store)
                if let Some(write) = rs.write {
                    if spilled_slices[write].is_some() {
                        // has been spilled before => check for dead store
                        if read_slices[write].is_none() {
                            // has not been read before => it's a dead store
                            dead_stores.push(inst_id);

                            // also remove all reads from this instruction since we eliminate it
                            for dead_store_read in rs.reads.iter() {
                                read_slices[*dead_store_read] = None;
                            }
                        } else {
                            // has been read before => no dead store, but remove from read set
                            read_slices[write] = None;
                        }
                    } else {
                        // has not been spilled before => mark slice as spill
                        spilled_slices[write] = Some(inst_id);
                    }
                }

                // process read groups (add to set)
                for read in &rs.reads {
                    // it's okay to override existing reads since we only care about the
                    // earliest (i.e. latest when iterating from back to front)
                    let read_group_id = self.optimizer_ancillary.group_ids[*read].unwrap();
                    for read_id in self.optimizer_ancillary.overlap_groups[read_group_id].iter() {
                        read_slices[*read_id] = Some(inst_id);
                    }
                }
            }

            // block writes are the spills we stored before
            self.optimizer_ancillary.block_writes[block_id] = spilled_slices;

            // the remaining read slices have no corresponding write before them so
            // they must be read from the previous block
            self.optimizer_ancillary.block_reads[block_id] = read_slices;
            self.optimizer_ancillary.dead_stores[block_id] = dead_stores;
        }

        // step 2 (fixed-point iteration)

        Ok(())
    }
}

/// Extract the read and write slice operands from the given `inst` while interning
/// any newly encountered slices in `slice_to_id` and `slice_ids`. The order of
/// reads and writes in [ReferencedSlices] must be stable.
pub(super) fn get_referenced_slices(
    inst: &JIRInstruction,
    slice_to_id: &mut FnvHashMap<JIRSlice, LocalSliceId>,
    slice_ids: &mut PrimaryMap<LocalSliceId, JIRSlice>,
) -> ReferencedSlices {
    let slice_write = inst
        .get_output()
        .map(|write| match slice_to_id.entry(*write) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                let slice_id = slice_ids.insert(*write);
                vacant_entry.insert(slice_id);
                slice_id
            }
        });

    let slice_reads = inst
        .operands()
        .filter_map(|value| match value {
            JIRValue::Slice(read) => {
                match slice_to_id.entry(*read) {
                    Entry::Occupied(occupied_entry) => Some(*occupied_entry.get()),
                    Entry::Vacant(vacant_entry) => {
                        // assign id and process overlap
                        let slice_id = slice_ids.insert(*read);
                        vacant_entry.insert(slice_id);
                        Some(slice_id)
                    }
                }
            }
            JIRValue::Const(_) => None,
        })
        .collect_vec();

    ReferencedSlices::new(slice_write, slice_reads)
}

new_map_key! { pub struct LocalSliceId; "LSID"; }
new_map_key! { pub struct OverlapGroupId; "OSID"; }
