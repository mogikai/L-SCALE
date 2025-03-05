//! This module contains all the functions that are used to emit code for [JIRInstruction]s

mod arithmetic;
mod bits;
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
pub use mmu::TlbMissCallback;
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
use crate::jit::x86::registers::RegisterTracker;
use crate::jit::x86::{
    X86GlobalContext,
    assemble,
};
pub use host_call::CustomOpCallback;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::jcode::{
    JIRInstruction,
    JIROperation,
};
use mogi_utilities::addresses::EVAddr;

pub struct X64Emitter<'a> {
    global_context: &'a X86GlobalContext,
    processor_spec: &'a ProcessorSpecification,
    register_tracker: &'a mut RegisterTracker,
    block: &'a mut CompiledBasicBlock<X64Relocation>,
}

impl X64Emitter<'_> {
    pub fn new<'a>(
        global_context: &'a X86GlobalContext,
        processor_spec: &'a ProcessorSpecification,
        register_tracker: &'a mut RegisterTracker,
        block: &'a mut CompiledBasicBlock<X64Relocation>,
    ) -> X64Emitter<'a> {
        X64Emitter::<'a> {
            global_context,
            processor_spec,
            register_tracker,
            block,
        }
    }

    pub(crate) fn emit_jinst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> JITResult {
        trace!("Emitting {jinst}");
        self.register_tracker.new_alloc_version();
        // TODO: Maybe we can "allocate" the registers ahead of time and provide some hints
        //       or something similar. Allocating the registers when having a complete
        //       view on all relevant instructions may be more efficient.
        let output = jinst.output.ok_or(RuntimeError::MissingOutput);
        let input_a = jinst.input_a.ok_or(RuntimeError::MissingInput);
        let input_b = jinst.input_b.ok_or(RuntimeError::MissingInput);
        match jinst.operation {
            JIROperation::BlockStart(block_id) => return Ok(()),
            JIROperation::BlockEnd(block_id) => return Ok(()),
            JIROperation::InstructionStart(addr) => return Ok(()),
            JIROperation::InstructionEnd(addr) => return Ok(()),
            JIROperation::Copy => return self.emit_copy(output?, input_a?),
            JIROperation::Load(space_id) => return self.emit_load(output?, input_a?, space_id),
            JIROperation::Store(space_id) => return self.emit_store(input_a?, space_id, input_b?),
            JIROperation::IntAdd => return self.emit_add(output?, input_a?, input_b?),
            JIROperation::IntSubtract => return self.emit_subtract(output?, input_a?, input_b?),
            JIROperation::IntMul => return self.emit_multiply(output?, input_a?, input_b?),
            JIROperation::IntDiv => return self.emit_divide(output?, input_a?, input_b?),
            JIROperation::IntSDiv => return self.emit_signed_divide(output?, input_a?, input_b?),
            JIROperation::IntRemainder => return self.emit_remainder(output?, input_a?, input_b?),
            JIROperation::IntSRemainder => return self.emit_signed_remainder(output?, input_a?, input_b?),
            JIROperation::IntLeft => return self.emit_shift_left(output?, input_a?, input_b?),
            JIROperation::IntRight => return self.emit_shift_right(output?, input_a?, input_b?),
            JIROperation::IntSRight => return self.emit_shift_right_signed(output?, input_a?, input_b?),
            JIROperation::IntOr => return self.emit_bitwise_or(output?, input_a?, input_b?),
            JIROperation::IntAnd => return self.emit_bitwise_and(output?, input_a?, input_b?),
            JIROperation::IntXor => return self.emit_bitwise_xor(output?, input_a?, input_b?),
            JIROperation::IntSLess => return self.emit_compare_lt_signed(output?, input_a?, input_b?),
            JIROperation::IntSGreater => return self.emit_compare_gt_signed(output?, input_a?, input_b?),
            JIROperation::IntSLessEqual => return self.emit_compare_le_signed(output?, input_a?, input_b?),
            JIROperation::IntSGreaterEqual => return self.emit_compare_ge_signed(output?, input_a?, input_b?),
            JIROperation::IntLess => return self.emit_compare_lt(output?, input_a?, input_b?),
            JIROperation::IntGreater => return self.emit_compare_gt(output?, input_a?, input_b?),
            JIROperation::IntLessEqual => return self.emit_compare_le(output?, input_a?, input_b?),
            JIROperation::IntGreaterEqual => return self.emit_compare_ge(output?, input_a?, input_b?),
            JIROperation::IntEquals => return self.emit_compare_eq(output?, input_a?, input_b?),
            JIROperation::IntNotEquals => return self.emit_compare_ne(output?, input_a?, input_b?),
            JIROperation::BitNegate => return self.emit_bitwise_negate(output?, input_a?),
            JIROperation::IntNegate => return self.emit_negate(output?, input_a?),
            JIROperation::FloatDiv => {}
            JIROperation::FloatMul => {}
            JIROperation::FloatAdd => {}
            JIROperation::FloatSubtract => {}
            JIROperation::FloatLess => {}
            JIROperation::FloatGreater => {}
            JIROperation::FloatLessEqual => {}
            JIROperation::FloatGreaterEqual => {}
            JIROperation::FloatEquals => {}
            JIROperation::FloatNotEquals => {}
            JIROperation::FloatNegate => {}
            JIROperation::BoolOr => return self.emit_boolean_or(output?, input_a?, input_b?),
            JIROperation::BoolAnd => return self.emit_boolean_and(output?, input_a?, input_b?),
            JIROperation::BoolXor => return self.emit_boolean_xor(output?, input_a?, input_b?),
            JIROperation::BoolNegate => return self.emit_boolean_negate(output?, input_a?),
            JIROperation::CallDirect => {}
            JIROperation::CallIndirect => {}
            JIROperation::ReturnDirect => {}
            JIROperation::ReturnIndirect => {}
            JIROperation::GotoDirect => return self.emit_goto_direct(input_a?),
            JIROperation::GotoIndirect => return self.emit_goto_indirect(input_a?),
            JIROperation::GotoDirectIf => return self.emit_goto_direct_if(input_a?, input_b?),
            JIROperation::GotoIndirectIf => {}
            JIROperation::JCodeGoto(block_id) => {}
            JIROperation::JCodeGotoIf(block_id) => {}
            JIROperation::PopCount => return self.emit_popcount(output?, input_a?),
            JIROperation::LZCount => return self.emit_lzcount(output?, input_a?),
            JIROperation::ZeroExtend => return self.emit_zero_extend(output?, input_a?),
            JIROperation::SignExtend => return self.emit_sign_extend(output?, input_a?),
            JIROperation::Carry => return self.emit_carry(output?, input_a?, input_b?),
            JIROperation::SignCarry => return self.emit_scarry(output?, input_a?, input_b?),
            JIROperation::SignBorrow => return self.emit_sborrow(output?, input_a?, input_b?),
            JIROperation::Nan => {}
            JIROperation::Abs => {}
            JIROperation::Sqrt => {}
            JIROperation::IntToFloat => {}
            JIROperation::FloatToFloat => {}
            JIROperation::Trunc => {}
            JIROperation::Ceil => {}
            JIROperation::Floor => {}
            JIROperation::Round => {}
            JIROperation::CustomOp { op_id, ref extra_args } => {
                return self.emit_custom_op_call(op_id, output, input_a, input_b, extra_args.as_slice());
            }
        }
        todo!("Instruction not yet implemented: {jinst}")
    }

    pub(crate) fn finalize(mut self) -> JITResult {
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
}
