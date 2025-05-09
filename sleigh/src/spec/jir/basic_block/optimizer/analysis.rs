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

use super::CopyCoalescion;
use super::copy_coalescing::Rewrite;

#[derive(Clone, Debug)]
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
            get_referenced_slices(
                inst,
                &mut self.optimizer_ancillary.slice_to_id,
                &mut self.optimizer_ancillary.slice_ids,
            );
        }

        Ok(())
    }

    /// Store a list of overlapping slices for each slice.
    pub(super) fn collect_slice_overlaps(&mut self) -> ProcessorSpecResult {
        for (slice_id, slice) in self.optimizer_ancillary.slice_ids.items() {
            let mut overlap_group = FnvHashSet::default();
            overlap_group.insert(slice_id);

            for (neighbor_candidate_id, neighbor_candidate) in self.optimizer_ancillary.slice_ids.items() {
                if neighbor_candidate_id == slice_id {
                    // own id is also in there => skip
                    continue;
                }

                // if they don't belong to the same group they can't overlap
                if slice.same_group(neighbor_candidate) && slice.overlaps(neighbor_candidate) {
                    // overlap

                    // add neighbor to own overlap group group
                    overlap_group.insert(neighbor_candidate_id);
                }
            }

            // check if overlap groups of slice and any of the neighbors is the same and if so,
            // reuse
            let mut group_id = None;
            for neighbor_id in overlap_group.iter() {
                if *neighbor_id == slice_id {
                    // own id is also in there => skip
                    continue;
                }

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

            for (inst_id, inst) in self.jir_instructions[block.range.clone()]
                .iter()
                .enumerate()
                .map(|(id, inst)| (id + block.range.start, inst))
                .rev()
            {
                let rs = get_referenced_slices(
                    inst,
                    &mut self.optimizer_ancillary.slice_to_id,
                    &mut self.optimizer_ancillary.slice_ids,
                );

                self.optimizer_ancillary
                    .inst_slice_references
                    .insert(inst_id, rs.clone());

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

    /// Calculate copy coalescions by iterating from start to end and eliminating
    /// all non-spilling slice copies.
    pub(super) fn calculate_copy_coalescions(&mut self) -> ProcessorSpecResult {
        for (block_id, block) in self.jir_blocks.items() {
            let Some(block) = block else {
                panic!("This must be Some");
            };

            // TODO handle slice overlaps (just ignore overlapping copies? special handling?
            // partial copies are kinda ugly)

            // copy_values can be constants if they are propagated or slices
            // if they originate from operations or previous block writes
            let mut coalescions = SecondaryMap::<LocalSliceId, Option<CopyCoalescion>>::default();

            // coalescions are finalized once they are overwritten
            let mut finalized_coalescions = Vec::new();

            for (inst_id, inst) in self.jir_instructions[block.range.clone()]
                .iter()
                .enumerate()
                .map(|(id, inst)| (id + block.range.start, inst))
            {
                match inst {
                    JIRInstruction::Copy(args) => {
                        // a copy is stored for later optimization
                        let copy_output_id = self
                            .optimizer_ancillary
                            .slice_to_id
                            .get(&args.output)
                            .expect("We stored this before");

                        // remove any copy coalescions for which the target value
                        // is equal to the write output
                        for coalescion in coalescions.values_mut() {
                            if let Some(coalescion_some) = coalescion
                                && let JIRValue::Slice(slice) = coalescion_some.value
                            {
                                let id = self
                                    .optimizer_ancillary
                                    .slice_to_id
                                    .get(&slice)
                                    .expect("We did this before");

                                if *id == *copy_output_id {
                                    *coalescion = None;
                                }
                            }
                        }

                        // finalize any coalescions that are overwritten by this instruction
                        let prev_coalescion = std::mem::take(&mut coalescions[*copy_output_id]);

                        if let Some(prev_coalescion) = prev_coalescion {
                            if prev_coalescion.rewrites.is_empty() {
                                // we don't care about empty coalescions
                                continue;
                            }

                            finalized_coalescions.push(prev_coalescion);
                        }

                        match args.input {
                            JIRValue::Slice(input_slice) => {
                                // an input slice might refer to another slice that
                                // was previously stored so we check for that first
                                let input_slice_id = self
                                    .optimizer_ancillary
                                    .slice_to_id
                                    .get(&input_slice)
                                    .expect("We stored this before");

                                let value = match &coalescions[*input_slice_id] {
                                    Some(prev_coalescion) => {
                                        // previously stored => use it
                                        prev_coalescion.value
                                    }
                                    None => {
                                        // not previously stored => new value (the slice)
                                        input_slice.into()
                                    }
                                };

                                let new_coalescion = CopyCoalescion::new(inst_id, value);
                                coalescions[*copy_output_id] = Some(new_coalescion);
                            }
                            JIRValue::Const(input_const) => {
                                // an input const can be stored as a copy_value as-is
                                let new_coalescion = CopyCoalescion::new(inst_id, input_const.into());
                                coalescions[*copy_output_id] = Some(new_coalescion);
                            }
                        }
                    }
                    inst => {
                        // any other instruction can trigger a rewrite if one of its reads has been
                        // encountered before in a copy
                        let rs = get_referenced_slices(
                            inst,
                            &mut self.optimizer_ancillary.slice_to_id,
                            &mut self.optimizer_ancillary.slice_ids,
                        );
                        for (readi, read) in rs.reads.into_iter().enumerate() {
                            if let Some(coalescion) = &mut coalescions[read] {
                                // if read was stored before as a coalescion, add
                                // it to the list of rewrites
                                coalescion.rewrites.push(Rewrite::new(inst_id, readi));
                            }
                        }

                        // S0 = S1
                        // S2 = S1
                        // S1 = 3 + S4

                        if let Some(write) = rs.write {
                            // remove any copy coalescions for which the target value
                            // is equal to the write output
                            for coalescion in coalescions.values_mut() {
                                if let Some(coalescion_some) = coalescion
                                    && let JIRValue::Slice(slice) = coalescion_some.value
                                {
                                    let id = self
                                        .optimizer_ancillary
                                        .slice_to_id
                                        .get(&slice)
                                        .expect("We did this before");

                                    if *id == write {
                                        *coalescion = None;
                                    }
                                }
                            }

                            // finalize any coalescions that are overwritten by this instruction
                            let prev_coalescion = std::mem::take(&mut coalescions[write]);

                            if let Some(prev_coalescion) = prev_coalescion {
                                if prev_coalescion.rewrites.is_empty() {
                                    // we don't care about empty coalescions
                                    continue;
                                }

                                finalized_coalescions.push(prev_coalescion);
                            }
                        }
                    }
                }
            }

            // all remaining elements in coalescions have no overwrite which means
            // they are spilled => we finalize them, but don't eliminate the copy
            // instruction
            for (_, remaining_coalescion) in coalescions.into_items() {
                if let Some(mut remaining_coalescion) = remaining_coalescion {
                    if remaining_coalescion.rewrites.is_empty() {
                        // we don't care about empty coalescions
                        continue;
                    }

                    remaining_coalescion.delete_copy_inst = false;
                    finalized_coalescions.push(remaining_coalescion);
                }
            }

            self.optimizer_ancillary.copy_coalescions[block_id] = finalized_coalescions;
        }

        Ok(())
    }
}

/// Extract the read and write slice operands from the given `inst` while interning
/// any newly encountered slices in `slice_to_id` and `slice_ids`. The order of
/// reads and writes in [ReferencedSlices] must be stable.
fn get_referenced_slices(
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
