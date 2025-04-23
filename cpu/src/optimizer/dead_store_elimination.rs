//! Dead store elimination optimizer.
//! This optimizer removes any stores that are never read.
//! Any varnode that is written to before it has been read is considered dead.
//! Varnodes that have not been read before the end of the block must always be
//! kept, since we don't know if they will be accessed later on.

use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

use crate::jit::api::JITResult;

struct ReferencedSlices {
    read: [Option<JIRSlice>; 2],
    written: Option<JIRSlice>,
}

impl ReferencedSlices {
    fn new(
        read: [Option<JIRSlice>; 2],
        written: Option<JIRSlice>,
    ) -> Self {
        Self { read, written }
    }
}

pub(super) fn eliminate_dead_stores(block: TargetBasicBlock) -> JITResult<TargetBasicBlock> {
    // Instructions that store values that are neither written nor spilled can
    // be determined by a single reverse pass and discarded.

    let mut dse_instructions = Vec::with_capacity(block.instruction_count());

    // The first time a slice is written results in a spill which we need to track.
    let mut spilled_slices = Vec::<JIRSlice>::new();

    // If a slice is written and has no read entry, we know it's a dead store.
    let mut read_slices = Vec::<JIRSlice>::new();

    for instruction in block.instructions().rev() {
        let rs = get_referenced_slices(instruction);

        // the written slice determines whether the instruction can stay or not
        if let Some(written) = rs.written {
            // check if written has been spilled
            match spilled_slices
                .iter_mut()
                .find(|spilled_slice| spilled_slice.overlaps(&written))
            {
                Some(spilled) => {
                    // written has been spilled => check if it is a smaller spill and if so, adjust
                    if spilled.is_strict_subset_of(&written) {
                        // smaller spill => adjust and keep
                        *spilled = spilled.union(&written);
                        dse_instructions.push(instruction.clone());
                    } else {
                        // larger spill => check if it has been read
                        if read_slices
                            .iter()
                            .any(|read_slice| read_slice.overlaps(&written))
                        {
                            // written has been read => keep it but remove all overlapping read slices
                            // since it got overwritten
                            read_slices.retain(|read_slice| !read_slice.overlaps(&written));
                            dse_instructions.push(instruction.clone());
                        } else {
                            // written has not been read => it's a dead store; do nothing in this case
                        }
                    }
                }
                None => {
                    // written has not been spilled => add it to the spill list and keep instruction
                    spilled_slices.push(written);
                    dse_instructions.push(instruction.clone());
                }
            }
        }

        // keep track of read registers only after the written ones have been taken care of
        for read_slice in rs.read.iter().filter_map(|read| *read) {
            read_slices.push(read_slice);
        }
    }

    dse_instructions.reverse();
    Ok(block.clone_with_instructions(dse_instructions))
}

fn get_referenced_slices(instr: &JIRInstruction) -> ReferencedSlices {
    match instr {
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
            let input_a = match binary_args.input_a {
                JIRValue::Slice(jirslice) => Some(jirslice),
                JIRValue::Const(_) => None,
            };

            let input_b = match binary_args.input_b {
                JIRValue::Slice(jirslice) => Some(jirslice),
                JIRValue::Const(_) => None,
            };

            let output = binary_args.output;

            ReferencedSlices::new([input_a, input_b], Some(output))
        }
        JIRInstruction::BlockStart(_)
        | JIRInstruction::BlockEnd(_)
        | JIRInstruction::InstructionStart(_)
        | JIRInstruction::InstructionEnd(_) => ReferencedSlices::new([None, None], None),
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
            let input = match unary_args.input {
                JIRValue::Slice(jirslice) => Some(jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([input, None], Some(unary_args.output))
        }
        JIRInstruction::Load {
            target,
            address,
            space: _,
        } => {
            let addr = match address {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([addr, None], Some(*target))
        }
        JIRInstruction::Store {
            address,
            value,
            space: _,
        } => {
            let addr = match address {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            let val = match value {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([addr, val], None)
        }
        JIRInstruction::CallDirect(jirvalue, _)
        | JIRInstruction::CallIndirect(jirvalue, _)
        | JIRInstruction::ReturnDirect(jirvalue)
        | JIRInstruction::ReturnIndirect(jirvalue) => {
            let input = match jirvalue {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([input, None], None)
        }
        JIRInstruction::GotoDirect(jirvalue) | JIRInstruction::GotoIndirect(jirvalue) => {
            let target = match jirvalue {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([target, None], None)
        }
        JIRInstruction::GotoDirectIf { condition, target } | JIRInstruction::GotoIndirectIf { condition, target } => {
            let cond = match condition {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            let tgt = match target {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([cond, tgt], None)
        }
        JIRInstruction::JCodeGoto(_) => ReferencedSlices::new([None, None], None),
        JIRInstruction::JCodeGotoIf {
            condition,
            target_block: _,
        } => {
            let cond = match condition {
                JIRValue::Slice(jirslice) => Some(*jirslice),
                JIRValue::Const(_) => None,
            };
            ReferencedSlices::new([cond, None], None)
        }
        JIRInstruction::CustomOp {
            output,
            input_a,
            input_b,
            op_id: _,
            extra_args: _,
        } => {
            let in_a = input_a.and_then(|input_a| match input_a {
                JIRValue::Slice(jirslice) => Some(jirslice),
                JIRValue::Const(_) => None,
            });
            let in_b = input_b.and_then(|input_b| match input_b {
                JIRValue::Slice(jirslice) => Some(jirslice),
                JIRValue::Const(_) => None,
            });
            ReferencedSlices::new([in_a, in_b], *output)
        }
    }
}
