//! This module contains all the functions that are used to emit code for [JIRInstruction]s

mod arithmetic;
mod bits;
mod copy;
mod extend;
mod flags;
mod flow;
mod host_call;
mod logic;
mod mmu;
pub(crate) mod regalloc;
pub(super) mod slice_load;
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
use crate::jit::host::register::HostRegSet;
use crate::jit::regalloc::RegisterAllocator;
use crate::jit::regalloc::requirements::{
    EmitterAllocRequirements,
    OperandRequest,
};
use crate::jit::regalloc::result::AllocatorResult;
use crate::jit::x86::registers::RegisterTracker;
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
    JIRSlice,
    JIRValue,
    JIRVarNodeId,
};
use mogi_utilities::addresses::EVAddr;

pub struct X64Emitter<'a> {
    global_context: &'a X86GlobalContext,
    processor_spec: &'a ProcessorSpecification,
    register_tracker: &'a mut RegisterTracker,

    /// The block to emit instructions into.
    block: &'a mut CompiledBasicBlock<X64Relocation>,

    /// The block the emitter should emit instructions for.
    target_block: &'a TargetBasicBlock,

    alloc_requirements: X64AllocRequirements,
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
}

impl X64AllocRequirements {
    pub fn new(target_block: &TargetBasicBlock) -> Self {
        let mut slf = Self {
            // Let's pessimistically allocate at least inst_count times two space.
            // This may need adjustment based on further testing.
            operand_requests: Vec::with_capacity(target_block.instructions().len() * 2),
            instruction_operand_ranges: Vec::with_capacity(target_block.instructions().len()),
            clobbers: Vec::with_capacity(target_block.instructions().len()),
        };
        slf.calculate_inst_requirements(target_block);
        slf
    }

    /// Calculates the requirements for a given target basic block.
    fn calculate_inst_requirements(
        &mut self,
        target_block: &TargetBasicBlock,
    ) {
        // TODO: Potentially, this step could be put into the IR generation step, so that we can directly add
        //       the requirements when the instruction is inserted, avoiding one iteration.
        for inst in target_block.instructions() {
            let range = self.add_instruction_operands(inst);
            self.instruction_operand_ranges.push(range);
        }
    }

    fn add_instruction_operands(
        &mut self,
        inst: &JIRInstruction,
    ) -> Range<usize> {
        let start = self.operand_requests.len();
        let clobberes = self.append_inst_operands(inst);
        self.clobbers.push(clobberes);
        let end = self.operand_requests.len();
        start..end
    }

    #[must_use]
    fn append_inst_operands(
        &mut self,
        inst: &JIRInstruction,
    ) -> HostRegSet {
        match inst {
            JIRInstruction::BlockStart(_) => HostRegSet::empty(),
            JIRInstruction::BlockEnd(_) => HostRegSet::empty(),
            JIRInstruction::InstructionStart(_) => HostRegSet::empty(),
            JIRInstruction::InstructionEnd(_) => HostRegSet::empty(),
            JIRInstruction::Copy(args) => self.append_copy_operands(args.output, args.input),
            JIRInstruction::Load { target, address, space } => self.append_load_operands(*target, *address, *space),
            JIRInstruction::Store { address, value, space } => self.append_store_operands(*address, *space, *value),
            JIRInstruction::IntAdd(args) => HostRegSet::empty(),
            JIRInstruction::IntSubtract(args) => HostRegSet::empty(),
            JIRInstruction::IntMul(_) => HostRegSet::empty(),
            JIRInstruction::IntDiv(_) => HostRegSet::empty(),
            JIRInstruction::IntSDiv(_) => HostRegSet::empty(),
            JIRInstruction::IntRemainder(_) => HostRegSet::empty(),
            JIRInstruction::IntSRemainder(_) => HostRegSet::empty(),
            JIRInstruction::IntLeft(_) => HostRegSet::empty(),
            JIRInstruction::IntRight(_) => HostRegSet::empty(),
            JIRInstruction::IntSRight(_) => HostRegSet::empty(),
            JIRInstruction::IntOr(_) => HostRegSet::empty(),
            JIRInstruction::IntAnd(_) => HostRegSet::empty(),
            JIRInstruction::IntXor(_) => HostRegSet::empty(),
            JIRInstruction::IntSLess(_) => HostRegSet::empty(),
            JIRInstruction::IntSGreater(_) => HostRegSet::empty(),
            JIRInstruction::IntSLessEqual(_) => HostRegSet::empty(),
            JIRInstruction::IntSGreaterEqual(_) => HostRegSet::empty(),
            JIRInstruction::IntLess(_) => HostRegSet::empty(),
            JIRInstruction::IntGreater(_) => HostRegSet::empty(),
            JIRInstruction::IntLessEqual(_) => HostRegSet::empty(),
            JIRInstruction::IntGreaterEqual(_) => HostRegSet::empty(),
            JIRInstruction::IntEquals(_) => HostRegSet::empty(),
            JIRInstruction::IntNotEquals(_) => HostRegSet::empty(),
            JIRInstruction::BitNegate(_) => HostRegSet::empty(),
            JIRInstruction::IntNegate(_) => HostRegSet::empty(),
            JIRInstruction::FloatDiv(_) => HostRegSet::empty(),
            JIRInstruction::FloatMul(_) => HostRegSet::empty(),
            JIRInstruction::FloatAdd(_) => HostRegSet::empty(),
            JIRInstruction::FloatSubtract(_) => HostRegSet::empty(),
            JIRInstruction::FloatLess(_) => HostRegSet::empty(),
            JIRInstruction::FloatGreater(_) => HostRegSet::empty(),
            JIRInstruction::FloatLessEqual(_) => HostRegSet::empty(),
            JIRInstruction::FloatGreaterEqual(_) => HostRegSet::empty(),
            JIRInstruction::FloatEquals(_) => HostRegSet::empty(),
            JIRInstruction::FloatNotEquals(_) => HostRegSet::empty(),
            JIRInstruction::FloatNegate(_) => HostRegSet::empty(),
            JIRInstruction::BoolOr(_) => HostRegSet::empty(),
            JIRInstruction::BoolAnd(_) => HostRegSet::empty(),
            JIRInstruction::BoolXor(_) => HostRegSet::empty(),
            JIRInstruction::BoolNegate(_) => HostRegSet::empty(),
            JIRInstruction::CallDirect(_) => HostRegSet::empty(),
            JIRInstruction::CallIndirect(_) => HostRegSet::empty(),
            JIRInstruction::ReturnDirect(_) => HostRegSet::empty(),
            JIRInstruction::ReturnIndirect(_) => HostRegSet::empty(),
            JIRInstruction::GotoDirect(_) => HostRegSet::empty(),
            JIRInstruction::GotoIndirect(_) => HostRegSet::empty(),
            JIRInstruction::GotoDirectIf { .. } => HostRegSet::empty(),
            JIRInstruction::GotoIndirectIf { .. } => HostRegSet::empty(),
            JIRInstruction::JCodeGoto(_) => HostRegSet::empty(),
            JIRInstruction::JCodeGotoIf { .. } => HostRegSet::empty(),
            JIRInstruction::PopCount(_) => HostRegSet::empty(),
            JIRInstruction::LZCount(_) => HostRegSet::empty(),
            JIRInstruction::ZeroExtend(_) => HostRegSet::empty(),
            JIRInstruction::SignExtend(_) => HostRegSet::empty(),
            JIRInstruction::Carry(_) => HostRegSet::empty(),
            JIRInstruction::SignCarry(_) => HostRegSet::empty(),
            JIRInstruction::SignBorrow(_) => HostRegSet::empty(),
            JIRInstruction::Nan(_) => HostRegSet::empty(),
            JIRInstruction::Abs(_) => HostRegSet::empty(),
            JIRInstruction::Sqrt(_) => HostRegSet::empty(),
            JIRInstruction::IntToFloat(_) => HostRegSet::empty(),
            JIRInstruction::FloatToFloat(_) => HostRegSet::empty(),
            JIRInstruction::Trunc(_) => HostRegSet::empty(),
            JIRInstruction::Ceil(_) => HostRegSet::empty(),
            JIRInstruction::Floor(_) => HostRegSet::empty(),
            JIRInstruction::Round(_) => HostRegSet::empty(),
            JIRInstruction::CustomOp { .. } => HostRegSet::empty(),
        }
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
    ) -> &[OperandRequest] {
        &self.operand_requests[self.instruction_operand_ranges[inst_id].clone()]
    }

    fn total_opereands(&self) -> usize {
        todo!()
    }
}

impl X64Emitter<'_> {
    pub fn new<'a>(
        global_context: &'a X86GlobalContext,
        processor_spec: &'a ProcessorSpecification,
        register_tracker: &'a mut RegisterTracker,
        block: &'a mut CompiledBasicBlock<X64Relocation>,
        target_block: &'a TargetBasicBlock,
    ) -> X64Emitter<'a> {
        X64Emitter {
            global_context,
            processor_spec,
            register_tracker,
            block,
            target_block,
            alloc_requirements: X64AllocRequirements::new(target_block),
        }
    }

    pub fn emitter_requirements(&self) -> &impl EmitterAllocRequirements {
        &self.alloc_requirements
    }

    pub fn emit_block(
        mut self,
        allocations: AllocatorResult,
    ) -> JITResult {
        for inst in self.target_block.instructions() {
            self.emit_inst(inst)?;
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
                self.emit_write_pc(u64::from(next).into())?;
                self.emit_flush_slices()?;
                assemble!(self.block.assembler();
                    ; jmp ->exit_success
                );
            }
        }
        Ok(())
    }

    fn emit_inst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> JITResult {
        trace!("Emitting {jinst}");
        self.register_tracker.new_alloc_version();
        // TODO: Maybe we can "allocate" the registers ahead of time and provide some hints
        //       or something similar. Allocating the registers when having a complete
        //       view on all relevant instructions may be more efficient.
        match jinst {
            JIRInstruction::BlockStart(block_id) => return Ok(()),
            JIRInstruction::BlockEnd(block_id) => return Ok(()),
            JIRInstruction::InstructionStart(addr) => return Ok(()),
            JIRInstruction::InstructionEnd(addr) => return Ok(()),
            JIRInstruction::Copy(args) => return self.emit_copy(args.output, args.input),
            JIRInstruction::Load { target, address, space } => return self.emit_load(*target, *address, *space),
            JIRInstruction::Store { address, value, space } => return self.emit_store(*address, *space, *value),
            JIRInstruction::IntAdd(args) => return self.emit_add(args.output, args.input_a, args.input_b),
            JIRInstruction::IntSubtract(args) => return self.emit_subtract(args.output, args.input_a, args.input_b),
            JIRInstruction::IntMul(args) => return self.emit_multiply(args.output, args.input_a, args.input_b),
            JIRInstruction::IntDiv(args) => return self.emit_divide(args.output, args.input_a, args.input_b),
            JIRInstruction::IntSDiv(args) => return self.emit_signed_divide(args.output, args.input_a, args.input_b),
            JIRInstruction::IntRemainder(args) => return self.emit_remainder(args.output, args.input_a, args.input_b),
            JIRInstruction::IntSRemainder(args) => {
                return self.emit_signed_remainder(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntLeft(args) => return self.emit_shift_left(args.output, args.input_a, args.input_b),
            JIRInstruction::IntRight(args) => return self.emit_shift_right(args.output, args.input_a, args.input_b),
            JIRInstruction::IntSRight(args) => {
                return self.emit_shift_right_signed(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntOr(args) => return self.emit_bitwise_or(args.output, args.input_a, args.input_b),
            JIRInstruction::IntAnd(args) => return self.emit_bitwise_and(args.output, args.input_a, args.input_b),
            JIRInstruction::IntXor(args) => return self.emit_bitwise_xor(args.output, args.input_a, args.input_b),
            JIRInstruction::IntSLess(args) => {
                return self.emit_compare_lt_signed(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntSGreater(args) => {
                return self.emit_compare_gt_signed(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntSLessEqual(args) => {
                return self.emit_compare_le_signed(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntSGreaterEqual(args) => {
                return self.emit_compare_ge_signed(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntLess(args) => return self.emit_compare_lt(args.output, args.input_a, args.input_b),
            JIRInstruction::IntGreater(args) => return self.emit_compare_gt(args.output, args.input_a, args.input_b),
            JIRInstruction::IntLessEqual(args) => return self.emit_compare_le(args.output, args.input_a, args.input_b),
            JIRInstruction::IntGreaterEqual(args) => {
                return self.emit_compare_ge(args.output, args.input_a, args.input_b);
            }
            JIRInstruction::IntEquals(args) => return self.emit_compare_eq(args.output, args.input_a, args.input_b),
            JIRInstruction::IntNotEquals(args) => return self.emit_compare_ne(args.output, args.input_a, args.input_b),
            JIRInstruction::BitNegate(args) => return self.emit_bitwise_negate(args.output, args.input),
            JIRInstruction::IntNegate(args) => return self.emit_negate(args.output, args.input),
            JIRInstruction::FloatDiv(args) => {}
            JIRInstruction::FloatMul(args) => {}
            JIRInstruction::FloatAdd(args) => {}
            JIRInstruction::FloatSubtract(args) => {}
            JIRInstruction::FloatLess(args) => {}
            JIRInstruction::FloatGreater(args) => {}
            JIRInstruction::FloatLessEqual(args) => {}
            JIRInstruction::FloatGreaterEqual(args) => {}
            JIRInstruction::FloatEquals(args) => {}
            JIRInstruction::FloatNotEquals(args) => {}
            JIRInstruction::FloatNegate(args) => {}
            JIRInstruction::BoolOr(args) => return self.emit_boolean_or(args.output, args.input_a, args.input_b),
            JIRInstruction::BoolAnd(args) => return self.emit_boolean_and(args.output, args.input_a, args.input_b),
            JIRInstruction::BoolXor(args) => return self.emit_boolean_xor(args.output, args.input_a, args.input_b),
            JIRInstruction::BoolNegate(args) => return self.emit_boolean_negate(args.output, args.input),
            JIRInstruction::CallDirect(_) => {}
            JIRInstruction::CallIndirect(_) => {}
            JIRInstruction::ReturnDirect(_) => {}
            JIRInstruction::ReturnIndirect(_) => {}
            JIRInstruction::GotoDirect(target) => return self.emit_goto_direct(*target),
            JIRInstruction::GotoIndirect(target) => return self.emit_goto_indirect(*target),
            JIRInstruction::GotoDirectIf { condition, target } => return self.emit_goto_direct_if(*target, *condition),
            JIRInstruction::GotoIndirectIf { condition, target } => {}
            JIRInstruction::JCodeGoto(block_id) => {}
            JIRInstruction::JCodeGotoIf {
                condition,
                target_block,
            } => {}
            JIRInstruction::PopCount(args) => return self.emit_popcount(args.output, args.input),
            JIRInstruction::LZCount(args) => return self.emit_lzcount(args.output, args.input),
            JIRInstruction::ZeroExtend(args) => return self.emit_zero_extend(args.output, args.input),
            JIRInstruction::SignExtend(args) => return self.emit_sign_extend(args.output, args.input),
            JIRInstruction::Carry(args) => return self.emit_carry(args.output, args.input_a, args.input_b),
            JIRInstruction::SignCarry(args) => return self.emit_scarry(args.output, args.input_a, args.input_b),
            JIRInstruction::SignBorrow(args) => return self.emit_sborrow(args.output, args.input_a, args.input_b),
            JIRInstruction::Nan(args) => {}
            JIRInstruction::Abs(args) => {}
            JIRInstruction::Sqrt(args) => {}
            JIRInstruction::IntToFloat(args) => {}
            JIRInstruction::FloatToFloat(args) => {}
            JIRInstruction::Trunc(args) => {}
            JIRInstruction::Ceil(args) => {}
            JIRInstruction::Floor(args) => {}
            JIRInstruction::Round(args) => {}
            JIRInstruction::CustomOp {
                output,
                input_a,
                input_b,
                op_id,
                extra_args,
            } => {
                return self.emit_custom_op_call(*op_id, *output, *input_a, *input_b, extra_args);
            }
        }
        todo!("Instruction not yet implemented: {jinst}")
    }
}
