//! This module contains all the functions that are used to emit code for [JIRInstruction]s

mod arithmetic;
mod bits;
mod copy;
mod directives;
mod extend;
mod flags;
mod flow;
mod helpers;
mod host_call;
mod logic;
mod mmu;
#[cfg(test)]
mod test;

use dynasmrt::x64::X64Relocation;
use either::Either;
pub use mmu::TlbMissCallback;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::ops::Range;
use tracing::trace;

/// Emit a 64 bit mov instruction, but only if the registers differ.
macro_rules! mov64 {
    ($asm:expr; $dest_reg:expr, $src_reg:expr) => {
        use dynasmrt::DynasmApi;
        use dynasmrt::dynasm;
        if $dest_reg != $src_reg {
            assemble!($asm;
                ; mov Rq($dest_reg), Rq($src_reg)
            );
        }
    };
}

use mov64;

use crate::jit::api::{
    JITResult,
    RuntimeError,
};
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::host::register::{
    HostRegMap,
    HostRegSet,
};
use crate::jit::regalloc::RegisterAllocator;
use crate::jit::regalloc::requirements::{
    EmitterAllocRequirements,
    OperandRequest,
    OperandRequirements,
    TempRequest,
};
use crate::jit::regalloc::result::{
    AllocatorResult,
    LoadSpillDirective,
};
use crate::jit::trace_jit;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::{
    X86GlobalContext,
    assemble,
};
pub use host_call::CustomOpCallback;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::basic_block::{
    InstId,
    TargetBasicBlock,
};
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
    JIRVarNodeId,
};
use mogi_utilities::addresses::EVAddr;

pub struct X64Emitter<'a> {
    allocations: &'a AllocatorResult,
    /// Keeps track of the slices currently loaded (tracked in forward direction).
    /// Any callback or external branch can then flush everything currently loaded.
    loaded_slices: HostRegMap<JIRSlice>,
    global_context: &'a X86GlobalContext,
    processor_spec: &'a ProcessorSpecification,

    /// The block to emit instructions into.
    block: &'a mut CompiledBasicBlock<X64Relocation>,

    /// The block the emitter should emit instructions for.
    target_block: &'a TargetBasicBlock,
}

/// This struct contains the data that is needed by the allocator.
/// It is calculated ahead of time to avoid having the individual requests
/// all over the place.
pub struct X64AllocRequirements {
    /// The operand requests for the instructions.
    /// The register allocator can query the emitter for each instruction.
    /// Each instruction has a consecutive slice of operands in this vec.
    /// The range is stored in `instruction_operand_ranges`
    operand_requests: Vec<OperandRequest>,

    /// Stores the range of consecutive operand requests in `operand_requests` for each instruction.
    instruction_operand_ranges: Vec<Range<usize>>,

    /// The clobber registers for each instruction.
    clobbers: Vec<HostRegSet>,
    /// contains the number of temps each instruction requires.
    required_temps: Vec<TempRequest>,
}

impl X64AllocRequirements {
    pub fn new(
        spec: &ProcessorSpecification,
        target_block: &TargetBasicBlock,
    ) -> Self {
        let mut slf = Self {
            // Let's pessimistically allocate at least inst_count times two space.
            // This may need adjustment based on further testing.
            operand_requests: Vec::with_capacity(target_block.instructions().len() * 2),
            instruction_operand_ranges: Vec::with_capacity(target_block.instructions().len()),
            clobbers: vec![HostRegSet::empty(); target_block.instructions().len()],
            required_temps: vec![TempRequest::new(); target_block.instructions().len()],
        };
        slf.calculate_inst_requirements(spec, target_block);
        slf
    }

    /// Calculates the requirements for a given target basic block.
    fn calculate_inst_requirements(
        &mut self,
        spec: &ProcessorSpecification,
        target_block: &TargetBasicBlock,
    ) {
        // TODO: Potentially, this step could be put into the IR generation step, so that we can directly add
        //       the requirements when the instruction is inserted, avoiding one iteration.
        for (inst_id, inst) in target_block.instructions_with_ids() {
            self.fetch_inst_operands(inst_id, spec, inst);
        }
    }

    fn fetch_inst_operands(
        &mut self,
        inst_id: InstId,
        spec: &ProcessorSpecification,
        inst: &JIRInstruction,
    ) {
        let mut requirement = AllocRequirement::new(
            inst_id,
            &mut self.operand_requests,
            &mut self.instruction_operand_ranges,
            &mut self.clobbers,
            &mut self.required_temps,
        );
        match inst {
            JIRInstruction::BlockStart(_) => {}
            JIRInstruction::BlockEnd(_) => {}
            JIRInstruction::InstructionStart(_) => {}
            JIRInstruction::InstructionEnd(_) => {}
            JIRInstruction::Copy(args) => Self::copy_requirements(&mut requirement, args.output, args.input),
            JIRInstruction::Load { target, address, space } => {
                Self::load_requirements(&mut requirement, *target, *address, *space)
            }
            JIRInstruction::Store { address, value, space } => {
                Self::store_requirements(&mut requirement, *address, *space, *value)
            }
            JIRInstruction::IntAdd(args) => {
                Self::add_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSubtract(args) => {
                Self::subtract_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntMul(args) => {
                Self::multiply_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntDiv(args) => {
                Self::divide_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSDiv(args) => {
                Self::divide_signed_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntRemainder(args) => {
                Self::remainder_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSRemainder(args) => {
                Self::signed_remainder_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntLeft(args) => {
                Self::shift_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntRight(args) => {
                Self::shift_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSRight(args) => {
                Self::shift_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntOr(args) => {
                Self::bitwise_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntAnd(args) => {
                Self::bitwise_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntXor(args) => {
                Self::bitwise_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSLess(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSGreater(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSLessEqual(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntSGreaterEqual(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntLess(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntGreater(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntLessEqual(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntGreaterEqual(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntEquals(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::IntNotEquals(args) => {
                Self::compare_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::BitNegate(args) => {
                Self::bitwise_negate_requirements(&mut requirement, args.output, args.input)
            }
            JIRInstruction::IntNegate(args) => Self::negate_requirements(&mut requirement, args.output, args.input),
            JIRInstruction::FloatDiv(_) => {}
            JIRInstruction::FloatMul(_) => {}
            JIRInstruction::FloatAdd(_) => {}
            JIRInstruction::FloatSubtract(_) => {}
            JIRInstruction::FloatLess(_) => {}
            JIRInstruction::FloatGreater(_) => {}
            JIRInstruction::FloatLessEqual(_) => {}
            JIRInstruction::FloatGreaterEqual(_) => {}
            JIRInstruction::FloatEquals(_) => {}
            JIRInstruction::FloatNotEquals(_) => {}
            JIRInstruction::FloatNegate(_) => {}
            JIRInstruction::BoolOr(args) => {
                Self::boolean_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::BoolAnd(args) => {
                Self::boolean_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::BoolXor(args) => {
                Self::boolean_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::BoolNegate(args) => {
                Self::boolean_negate_requirements(&mut requirement, args.output, args.input)
            }
            JIRInstruction::CallDirect(..) => {}
            JIRInstruction::CallIndirect(..) => {}
            JIRInstruction::ReturnDirect(_) => {}
            JIRInstruction::ReturnIndirect(_) => {}
            JIRInstruction::GotoDirect(target) => Self::goto_direct_requirements(&mut requirement, *target),
            JIRInstruction::GotoIndirect(target) => {
                let pc = spec.program_counter_slice();
                let space = spec.default_space();
                Self::goto_indirect_requirements(&mut requirement, pc, *target, space);
            }
            JIRInstruction::GotoDirectIf { condition, target } => {
                Self::goto_direct_if_requirements(&mut requirement, *condition, *target);
            }
            JIRInstruction::GotoIndirectIf { .. } => {}
            JIRInstruction::JCodeGoto(_) => {}
            JIRInstruction::JCodeGotoIf { .. } => {}
            JIRInstruction::PopCount(args) => {
                Self::popcount_requirements(&mut requirement, args.output, args.input);
            }
            JIRInstruction::LZCount(args) => {
                Self::lzcount_requirements(&mut requirement, args.output, args.input);
            }
            JIRInstruction::ZeroExtend(args) => {
                Self::zero_extend_requirements(&mut requirement, args.output, args.input)
            }
            JIRInstruction::SignExtend(args) => {
                Self::sign_extend_requirements(&mut requirement, args.output, args.input)
            }
            JIRInstruction::Carry(args) => {
                Self::carry_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::SignCarry(args) => {
                Self::scarry_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::SignBorrow(args) => {
                Self::sborrow_requirements(&mut requirement, args.output, args.input_a, args.input_b)
            }
            JIRInstruction::Nan(_) => {}
            JIRInstruction::Abs(_) => {}
            JIRInstruction::Sqrt(_) => {}
            JIRInstruction::IntToFloat(_) => {}
            JIRInstruction::FloatToFloat(_) => {}
            JIRInstruction::Trunc(_) => {}
            JIRInstruction::Ceil(_) => {}
            JIRInstruction::Floor(_) => {}
            JIRInstruction::Round(_) => {}
            JIRInstruction::CustomOp { .. } => {}
        }
    }

    pub fn make_emitter<'e>(
        &self,
        allocations: &'e AllocatorResult,
        global_context: &'e X86GlobalContext,
        processor_spec: &'e ProcessorSpecification,
        block: &'e mut CompiledBasicBlock<X64Relocation>,
        target_block: &'e TargetBasicBlock,
    ) -> X64Emitter<'e> {
        X64Emitter::<'e>::new(allocations, global_context, processor_spec, block, target_block)
    }
}

impl EmitterAllocRequirements for X64AllocRequirements {
    fn clobbers(
        &self,
        inst_id: InstId,
    ) -> HostRegSet {
        self.clobbers[inst_id]
    }

    fn operands(
        &self,
        inst_id: InstId,
    ) -> OperandRequirements {
        OperandRequirements::new(&self.operand_requests[self.instruction_operand_ranges[inst_id].clone()])
    }

    fn required_temps(
        &self,
        inst_id: InstId,
    ) -> TempRequest {
        self.required_temps[inst_id]
    }

    fn total_opereands(&self) -> usize {
        todo!()
    }
}

impl X64Emitter<'_> {
    fn new<'a>(
        allocations: &'a AllocatorResult,
        global_context: &'a X86GlobalContext,
        processor_spec: &'a ProcessorSpecification,
        block: &'a mut CompiledBasicBlock<X64Relocation>,
        target_block: &'a TargetBasicBlock,
    ) -> X64Emitter<'a> {
        X64Emitter {
            allocations,
            loaded_slices: Default::default(),
            global_context,
            processor_spec,
            block,
            target_block,
        }
    }

    pub fn emit_block(mut self) -> JITResult {
        for (inst_id, inst) in self.target_block.instructions_with_ids() {
            self.emit_inst(inst_id, inst)?;
        }
        self.finalize()
    }
    /// Writes the block epilogue that writes the program counter, flushes any slices, and jumps to the next block.
    fn finalize(mut self) -> JITResult {
        match self.block.fallthrough() {
            None => {
                // No successor specified. This means the processor is halting or something similar.
                // Jump to failure exit.
                // TODO: Write exception to runtime context.
                assemble!(self.block.assembler();
                    // Intentionally trigger an exception, since this case is a todo
                    ; mov rax, [0xfffff]
                );
            }
            Some(next) => {
                // Write the pc before we exit the block to make it visible to the jit.
                // TODO: This could maybe be optimized such that we do not write the pc on chained blocks?
                self.emit_write_pc(JIRConst::from(u64::from(next)).into())?;
                // self.save_loaded_slices()?;
                assemble!(self.block.assembler();
                    ; jmp ->exit_success
                );
            }
        }
        Ok(())
    }

    fn emit_inst(
        &mut self,
        inst_id: InstId,
        jinst: &JIRInstruction,
    ) -> JITResult {
        trace_jit!("Emitting {jinst}");
        // Remove any clobbers from the loaded slices.
        let clobbers = self.allocations.get_clobbers(inst_id);
        trace_jit!("Removing clobbers from loaded slices: {clobbers}");
        self.loaded_slices.remove_set(clobbers);
        self.emit_directives(
            self.allocations
                .directives()
                .before_inst_directives(inst_id),
        )?;
        match jinst {
            JIRInstruction::BlockStart(block_id) => {}
            JIRInstruction::BlockEnd(block_id) => {}
            JIRInstruction::InstructionStart(addr) => {}
            JIRInstruction::InstructionEnd(addr) => {}
            JIRInstruction::Copy(args) => self.emit_copy(inst_id, args.output, args.input)?,
            JIRInstruction::Load { target, address, space } => self.emit_load(inst_id, *target, *address, *space)?,
            JIRInstruction::Store { address, value, space } => self.emit_store(inst_id, *address, *space, *value)?,
            JIRInstruction::IntAdd(args) => self.emit_add(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntSubtract(args) => {
                self.emit_subtract(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntMul(args) => self.emit_multiply(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntDiv(args) => self.emit_divide(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntSDiv(args) => {
                self.emit_signed_divide(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntRemainder(args) => {
                self.emit_remainder(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntSRemainder(args) => {
                self.emit_signed_remainder(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntLeft(args) => self.emit_shift_left(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntRight(args) => {
                self.emit_shift_right(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntSRight(args) => {
                self.emit_shift_right_signed(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntOr(args) => self.emit_bitwise_or(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntAnd(args) => self.emit_bitwise_and(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntXor(args) => self.emit_bitwise_xor(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntSLess(args) => {
                self.emit_compare_lt_signed(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntSGreater(args) => {
                self.emit_compare_gt_signed(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntSLessEqual(args) => {
                self.emit_compare_le_signed(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntSGreaterEqual(args) => {
                self.emit_compare_ge_signed(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntLess(args) => self.emit_compare_lt(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::IntGreater(args) => {
                self.emit_compare_gt(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntLessEqual(args) => {
                self.emit_compare_le(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntGreaterEqual(args) => {
                self.emit_compare_ge(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntEquals(args) => {
                self.emit_compare_eq(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::IntNotEquals(args) => {
                self.emit_compare_ne(inst_id, args.output, args.input_a, args.input_b)?
            }
            JIRInstruction::BitNegate(args) => self.emit_bitwise_negate(inst_id, args.output, args.input)?,
            JIRInstruction::IntNegate(args) => self.emit_negate(inst_id, args.output, args.input)?,
            JIRInstruction::FloatDiv(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatMul(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatAdd(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatSubtract(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatLess(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatGreater(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatLessEqual(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatGreaterEqual(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatEquals(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatNotEquals(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatNegate(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::BoolOr(args) => self.emit_boolean_or(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::BoolAnd(args) => self.emit_boolean_and(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::BoolXor(args) => self.emit_boolean_xor(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::BoolNegate(args) => self.emit_boolean_negate(inst_id, args.output, args.input)?,
            JIRInstruction::CallDirect(..) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::CallIndirect(..) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::ReturnDirect(_) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::ReturnIndirect(_) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::GotoDirect(target) => self.emit_goto_direct(inst_id, *target)?,
            JIRInstruction::GotoIndirect(target) => self.emit_goto_indirect(inst_id, *target)?,
            JIRInstruction::GotoDirectIf { condition, target } => {
                self.emit_goto_direct_if(inst_id, *target, *condition)?
            }
            JIRInstruction::GotoIndirectIf { condition, target } => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::JCodeGoto(block_id) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::JCodeGotoIf {
                condition,
                target_block,
            } => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::PopCount(args) => self.emit_popcount(inst_id, args.output, args.input)?,
            JIRInstruction::LZCount(args) => self.emit_lzcount(inst_id, args.output, args.input)?,
            JIRInstruction::ZeroExtend(args) => self.emit_zero_extend(inst_id, args.output, args.input)?,
            JIRInstruction::SignExtend(args) => self.emit_sign_extend(inst_id, args.output, args.input)?,
            JIRInstruction::Carry(args) => self.emit_carry(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::SignCarry(args) => self.emit_scarry(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::SignBorrow(args) => self.emit_sborrow(inst_id, args.output, args.input_a, args.input_b)?,
            JIRInstruction::Nan(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Abs(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Sqrt(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::IntToFloat(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::FloatToFloat(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Trunc(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Ceil(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Floor(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::Round(args) => todo!("Instruction not yet implemented: {jinst}"),
            JIRInstruction::CustomOp {
                output,
                input_a,
                input_b,
                op_id,
                extra_args,
            } => {
                self.emit_custom_op_call(*op_id, *output, *input_a, *input_b, extra_args)?;
            }
        }

        // We don't need to emit directives on unconditional branches since the jump is emitted beforehand.
        if !jinst.is_unconditional_branch() {
            self.emit_directives(self.allocations.directives().after_inst_directives(inst_id))?;
        }

        Ok(())
    }
}
