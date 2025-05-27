use std::collections::hash_map::Entry;

use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use itertools::Itertools;
use mogi_utilities::index_map::{
    MapKey,
    PrimaryMap,
    SecondaryMap,
};
use mogi_utilities::{
    new_map_key,
    overlap,
};

use crate::spec::jir::basic_block::{
    InstId,
    TargetBasicBlockInner,
};
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::values::{
    GroupId,
    JIRGroup,
    JIRSlice,
    JIRValue,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
};

use super::LocalSliceId;

impl TargetBasicBlockInner {
    /// Store a unique id for each slice and populate `self.group_slices`.
    pub(super) fn collect_slices(
        &mut self,
        spec: &ProcessorSpecification,
    ) -> ProcessorSpecResult {
        for inst in &self.jir_instructions {
            if let Some(write_slice) = inst.get_output() {
                let write_id = match self.optimizer_ancillary.slice_to_id.entry(write_slice) {
                    Entry::Occupied(occupied_entry) => *occupied_entry.get(),
                    Entry::Vacant(vacant_entry) => {
                        let slice_id = self.optimizer_ancillary.slice_ids.insert(write_slice);
                        vacant_entry.insert(slice_id);
                        slice_id
                    }
                };

                self.optimizer_ancillary.group_slices[write_slice.group_id].insert(write_id);
            }

            for read in inst.operands() {
                if let JIRValue::Slice(read_slice) = read {
                    let read_id = match self.optimizer_ancillary.slice_to_id.entry(*read_slice) {
                        Entry::Occupied(occupied_entry) => *occupied_entry.get(),
                        Entry::Vacant(vacant_entry) => {
                            let slice_id = self.optimizer_ancillary.slice_ids.insert(*read_slice);
                            vacant_entry.insert(slice_id);
                            slice_id
                        }
                    };

                    self.optimizer_ancillary.group_slices[read_slice.group_id].insert(read_id);
                }
            }
        }

        Ok(())
    }

    /// Populate `block_reads` and `block_writes` and  mark dead-store instructions.
    pub(super) fn calculate_block_reads_and_writes(
        &mut self,
        spec: &ProcessorSpecification,
    ) -> ProcessorSpecResult {
        // this is a two-phase algorithm:
        // 1. calculate reads and writes for each block locally
        // 2. use fixed-point iteration to solve cycles

        // step 1 (local blocks)
        for (block_id, block) in self.jir_blocks.items() {
            let Some(block) = block else {
                panic!("This must be Some");
            };

            let mut dead_stores = Vec::new();

            // In a local setting, we must assume all written slices are spilled.
            // We also store which instruction spilled them.
            let mut spilled_slices = SecondaryMap::<LocalSliceId, Option<InstId>>::default();

            // If a slice is written and has no read entry, we know it's a dead store.
            // We also store which instruction read it.
            let mut read_slices = SecondaryMap::<LocalSliceId, Option<InstId>>::default();

            for inst_id in block.range.clone().rev() {
                // process written slices (mark spills and check if current inst is dead store)
                self.mark_spills_and_dead_stores(inst_id, &mut spilled_slices, &mut read_slices, &mut dead_stores);

                // process read groups (add to set)
                for read in self.jir_instructions[inst_id].operands() {
                    let JIRValue::Slice(read_slice) = read else {
                        continue;
                    };

                    // it's okay to override existing reads since we only care about the
                    // earliest (i.e. latest when iterating from back to front)

                    // mark all varnodes from group as read (TODO exact overlap?)
                    for overlap_candidate in &self.optimizer_ancillary.group_slices[read_slice.group_id] {
                        read_slices[*overlap_candidate] = Some(inst_id);
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

    fn mark_spills_and_dead_stores(
        &self,
        inst_id: InstId,
        spilled_slices: &mut SecondaryMap<LocalSliceId, Option<InstId>>,
        read_slices: &mut SecondaryMap<LocalSliceId, Option<InstId>>,
        dead_stores: &mut Vec<InstId>,
    ) {
        if let Some(write) = self.jir_instructions[inst_id].get_output() {
            let write_id = self.optimizer_ancillary.slice_to_id[&write];

            if spilled_slices[write_id].is_some() {
                // has been spilled before => check for dead store
                if read_slices[write_id].is_none() {
                    // has not been read before => it's a dead store
                    dead_stores.push(inst_id);

                    // also remove all reads from this instruction since we eliminate it
                    for ds_read in self.jir_instructions[inst_id].operands() {
                        let JIRValue::Slice(ds_read_slice) = ds_read else {
                            continue;
                        };

                        let ds_read_slice_id = self.optimizer_ancillary.slice_to_id[&ds_read_slice];
                        // care: overlaps can only be removed
                        // if they are a strict subset of ds_read
                        // TODO is it worth removing those kinds of reads?
                        read_slices[ds_read_slice_id] = None;
                    }
                } else {
                    // has been read before => no dead store, but remove from read set
                    read_slices[write_id] = None;
                }
            } else {
                // has not been spilled before => mark slice as spill
                spilled_slices[write_id] = Some(inst_id);
            }
        }
    }
}

new_map_key! { pub struct OverlapGroupId; "OSID"; }
