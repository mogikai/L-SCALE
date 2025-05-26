use std::collections::hash_map::Entry;

use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use itertools::Itertools;
use mogi_utilities::index_map::PrimaryMap;
use mogi_utilities::new_map_key;

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::TargetBasicBlockInner;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

#[derive(Clone, Debug)]
pub(super) struct ReferencedSlices {
    writes: Vec<LocalSliceId>,
    reads: Vec<LocalSliceId>,
}

impl ReferencedSlices {
    fn new(
        writes: Vec<LocalSliceId>,
        reads: Vec<LocalSliceId>,
    ) -> Self {
        Self { writes, reads }
    }
}

impl TargetBasicBlockInner {
    /// Store a unique id for each slice and annotate instructions with their
    /// input and output slices..
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

    /// Populate `block_reads` and `block_writes` and mark dead-store instructions.
    pub(super) fn calculate_block_reads_and_writes(&mut self) -> ProcessorSpecResult {
        // this is a two-phase algorithm:
        // 1. calculate reads and writes for each block locally
        // 2. use fixed-point iteration to solve cycles

        // step 1 (local blocks)
        for (block_id, block) in self.jir_blocks.items() {
            let Some(block) = block else {
                panic!("This must be Some");
            };

            let mut dead_stores = vec![];

            // iterate over the instructions in reverse, storing

            // in a local setting, we must assume all written slices are spilled
            let mut written_slices = FnvHashSet::<LocalSliceId>::default();

            // if a slice is written and has no read entry, we know it's a dead store.
            let mut read_slices = FnvHashSet::<LocalSliceId>::default();

            for (inst_id, inst) in self.jir_instructions[block.range.clone()]
                .iter()
                .enumerate()
            {
                let rs = get_referenced_slices(
                    inst,
                    &mut self.optimizer_ancillary.slice_to_id,
                    &mut self.optimizer_ancillary.slice_ids,
                );

                // process written slices (check for spills and if current inst is dead store)
                for write in &rs.writes {
                    written_slices.insert(*write);

                    if !read_slices.contains(write) {
                        // written slice hasn't been read before previously => it's a dead store
                        dead_stores.push(inst_id);
                    } else {
                        // written slice has been read before => no dead store, but remove from
                        // read set
                        read_slices.remove(write);
                    }
                }

                // process read slices (add to set)
                for read in &rs.reads {
                    read_slices.insert(*read);
                }
            }

            self.optimizer_ancillary.dead_stores[block_id] = dead_stores;
        }

        // step 2 (fixed-point iteration)

        Ok(())
    }
}

fn get_referenced_slices(
    inst: &JIRInstruction,
    slice_to_id: &mut FnvHashMap<JIRSlice, LocalSliceId>,
    slice_ids: &mut PrimaryMap<LocalSliceId, JIRSlice>,
) -> ReferencedSlices {
    let (slice_writes, slice_reads) = match inst {
        JIRInstruction::IntMul(binary_args)
        | JIRInstruction::IntDiv(binary_args)
        | JIRInstruction::IntSDiv(binary_args)
        | JIRInstruction::IntAdd(binary_args)
        | JIRInstruction::IntSubtract(binary_args)
        | JIRInstruction::IntRemainder(binary_args)
        | JIRInstruction::IntSRemainder(binary_args)
        | JIRInstruction::IntLeft(binary_args)
        | JIRInstruction::IntRight(binary_args)
        | JIRInstruction::IntSRight(binary_args)
        | JIRInstruction::IntOr(binary_args)
        | JIRInstruction::IntAnd(binary_args)
        | JIRInstruction::IntXor(binary_args)
        | JIRInstruction::IntSLess(binary_args)
        | JIRInstruction::IntSGreater(binary_args)
        | JIRInstruction::IntSLessEqual(binary_args)
        | JIRInstruction::IntSGreaterEqual(binary_args)
        | JIRInstruction::IntLess(binary_args)
        | JIRInstruction::IntGreater(binary_args)
        | JIRInstruction::IntLessEqual(binary_args)
        | JIRInstruction::IntGreaterEqual(binary_args)
        | JIRInstruction::IntEquals(binary_args)
        | JIRInstruction::IntNotEquals(binary_args)
        | JIRInstruction::FloatDiv(binary_args)
        | JIRInstruction::FloatMul(binary_args)
        | JIRInstruction::FloatAdd(binary_args)
        | JIRInstruction::FloatSubtract(binary_args)
        | JIRInstruction::FloatLess(binary_args)
        | JIRInstruction::FloatGreater(binary_args)
        | JIRInstruction::FloatLessEqual(binary_args)
        | JIRInstruction::FloatGreaterEqual(binary_args)
        | JIRInstruction::FloatEquals(binary_args)
        | JIRInstruction::FloatNotEquals(binary_args)
        | JIRInstruction::BoolOr(binary_args)
        | JIRInstruction::BoolAnd(binary_args)
        | JIRInstruction::BoolXor(binary_args)
        | JIRInstruction::Carry(binary_args)
        | JIRInstruction::SignCarry(binary_args)
        | JIRInstruction::SignBorrow(binary_args) => {
            let writes = vec![binary_args.output];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = binary_args.input_a {
                reads.push(jirslice);
            }

            if let JIRValue::Slice(jirslice) = binary_args.input_b {
                reads.push(jirslice);
            }

            (writes, reads)
        }
        JIRInstruction::BlockStart(_)
        | JIRInstruction::BlockEnd(_)
        | JIRInstruction::InstructionStart(_)
        | JIRInstruction::InstructionEnd(_) => (vec![], vec![]),
        JIRInstruction::Copy(unary_args)
        | JIRInstruction::BitNegate(unary_args)
        | JIRInstruction::IntNegate(unary_args)
        | JIRInstruction::FloatNegate(unary_args)
        | JIRInstruction::BoolNegate(unary_args)
        | JIRInstruction::PopCount(unary_args)
        | JIRInstruction::LZCount(unary_args)
        | JIRInstruction::ZeroExtend(unary_args)
        | JIRInstruction::SignExtend(unary_args)
        | JIRInstruction::Nan(unary_args)
        | JIRInstruction::Abs(unary_args)
        | JIRInstruction::Sqrt(unary_args)
        | JIRInstruction::IntToFloat(unary_args)
        | JIRInstruction::FloatToFloat(unary_args)
        | JIRInstruction::Trunc(unary_args)
        | JIRInstruction::Ceil(unary_args)
        | JIRInstruction::Floor(unary_args)
        | JIRInstruction::Round(unary_args) => {
            let writes = vec![unary_args.output];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = unary_args.input {
                reads.push(jirslice);
            }

            (writes, reads)
        }
        JIRInstruction::Load {
            target,
            address,
            space: _,
        } => {
            let writes = vec![target.clone()];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = address {
                reads.push(jirslice.clone());
            }

            (writes, reads)
        }
        JIRInstruction::Store {
            address,
            value,
            space: _,
        } => {
            let mut writes = vec![];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = value {
                reads.push(jirslice.clone());
            }

            if let JIRValue::Slice(jirslice) = address {
                writes.push(jirslice.clone());
            }

            (writes, reads)
        }
        JIRInstruction::CallDirect(jirvalue, _)
        | JIRInstruction::CallIndirect(jirvalue, _)
        | JIRInstruction::ReturnDirect(jirvalue)
        | JIRInstruction::ReturnIndirect(jirvalue)
        | JIRInstruction::GotoDirect(jirvalue)
        | JIRInstruction::GotoIndirect(jirvalue) => {
            let writes = vec![];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = jirvalue {
                reads.push(jirslice.clone());
            }

            (writes, reads)
        }
        JIRInstruction::GotoDirectIf { condition, target } | JIRInstruction::GotoIndirectIf { condition, target } => {
            let writes = vec![];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = condition {
                reads.push(jirslice.clone());
            }

            if let JIRValue::Slice(jirslice) = target {
                reads.push(jirslice.clone());
            }

            (writes, reads)
        }
        JIRInstruction::JCodeGoto(_) => (vec![], vec![]),
        JIRInstruction::JCodeGotoIf {
            condition,
            target_block: _,
        } => {
            let writes = vec![];
            let mut reads = vec![];

            if let JIRValue::Slice(jirslice) = condition {
                reads.push(jirslice.clone());
            }

            (writes, reads)
        }
        JIRInstruction::CustomOp {
            output,
            input_a,
            input_b,
            op_id: _,
            extra_args: _,
        } => {
            let mut writes = vec![];
            let mut reads = vec![];

            if let Some(output) = output {
                writes.push(output.clone());
            }

            if let Some(JIRValue::Slice(jirslice)) = input_a {
                reads.push(jirslice.clone());
            }

            if let Some(JIRValue::Slice(jirslice)) = input_b {
                reads.push(jirslice.clone());
            }

            // TODO extra args?

            (writes, reads)
        }
    };

    let slice_writes = slice_writes
        .into_iter()
        .map(|write| match slice_to_id.entry(write) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                let slice_id = slice_ids.insert(write);
                vacant_entry.insert(slice_id);
                slice_id
            }
        })
        .collect_vec();

    let slice_reads = slice_reads
        .into_iter()
        .map(|read| match slice_to_id.entry(read) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                let slice_id = slice_ids.insert(read);
                vacant_entry.insert(slice_id);
                slice_id
            }
        })
        .collect_vec();

    ReferencedSlices::new(slice_writes, slice_reads)
}

new_map_key! { pub struct LocalSliceId; "LSID"; }
