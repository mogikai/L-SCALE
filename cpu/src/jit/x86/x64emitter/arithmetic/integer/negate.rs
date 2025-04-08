use crate::jit::api::JITResult;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn negate_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        match input {
            JIRValue::Slice(input) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input));
            }
            JIRValue::Const(input) => {
                Self::copy_requirements(requirement, output, input);
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit x86 assembly to perform an integer negation on the provided [JIRValue].
    /// The result is stored inside the output [JIRSlice].
    /// Providing a constant input will most likely result in a move of the already negated value.
    pub fn emit_negate(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        match input {
            JIRValue::Const(value) => self.emit_copy(
                inst,
                output,
                JIRConst::new((value.value() as i128).wrapping_neg() as u128, value.size()),
            )?,
            JIRValue::Slice(slice) => {
                // Copy into output, since neg uses the same register for input and output.
                // If output==input this is a no-op.
                let output_reg = reg!(self.allocations, inst, 0);
                let input_reg = reg!(self.allocations, inst, 1);

                self.copy_reg(output_reg, input_reg, output.size(), CopyExtendMode::None)?;
                assemble!(self.block.assembler();
                    ; neg Rq(output_reg.hw_enc())
                );
            }
        }
        Ok(())
    }
}
