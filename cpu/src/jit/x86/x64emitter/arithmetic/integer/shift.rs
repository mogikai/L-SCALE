use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::trace_jit;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
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
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn shift_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
    ) {
        match (input, shift) {
            (JIRValue::Const(_), JIRValue::Const(_)) => {
                // TODO: delegate to copy?
                requirement.add_request(OperandRequest::int_write_any(output));
            }
            (JIRValue::Const(_), JIRValue::Slice(shift)) => {
                // Shift is always loaded in cx on x86
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::CX.into_host_reg(), shift));
            }
            (JIRValue::Slice(input), JIRValue::Slice(shift)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input));
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::CX.into_host_reg(), shift));
            }
            (JIRValue::Slice(input), JIRValue::Const(_)) => {
                requirement.add_request(OperandRequest::int_write_any(output));
                requirement.add_request(OperandRequest::int_read_any(input));
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_shift_left(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input.size());
        if output.size() > 8 {
            todo!("Sizes greater than 8 are currently unsupported for shift operations.")
        }
        trace_jit!("Emitting shl {output:?} = {input:?} << {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, out_reg, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_reg.hw_enc()), cl
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_reg.hw_enc()), cl
                ),
                _ => todo!(),
            },
            |asm, out_reg, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_reg.hw_enc()), imm
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_reg.hw_enc()), imm
                ),
                _ => todo!(),
            },
        )
    }

    pub fn emit_shift_right(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input.size());
        if output.size() > 8 {
            todo!("Sizes greater than 8 are currently unsupported for shift operations.")
        }
        trace_jit!("Emitting shr {output:?} = {input:?} >> {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, out_reg, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_reg.hw_enc()), cl
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_reg.hw_enc()), cl
                ),
                _ => todo!(),
            },
            |asm, out_reg, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_reg.hw_enc()), imm
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_reg.hw_enc()), imm
                ),
                _ => todo!(),
            },
        )
    }

    pub fn emit_shift_right_signed(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
    ) -> JITResult {
        assert_eq!(output.size(), input.size());
        if output.size() > 8 {
            todo!("Sizes greater than 8 are currently unsupported for shift operations.")
        }
        trace_jit!("Emitting shr {output:?} = {input:?} s>> {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, out_reg, size| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_reg.hw_enc()), cl
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_reg.hw_enc()), cl
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_reg.hw_enc()), cl
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_reg.hw_enc()), cl
                ),
                _ => todo!("Unsupported shift size"),
            },
            |asm, out_reg, size, imm| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_reg.hw_enc()), imm
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_reg.hw_enc()), imm
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_reg.hw_enc()), imm
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_reg.hw_enc()), imm
                ),
                _ => todo!("Unsupported shift size"),
            },
        )
    }

    /// This function emits the setup code for a shift operation, i.e. loading registers.
    /// Then it calls one of the callbacks to emit the actual shift instruction.
    fn shift(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
        asm_reg: fn(&mut MogiAssembler<X64Relocation>, HostReg, size: JIRSize),
        asm_imm: fn(&mut MogiAssembler<X64Relocation>, HostReg, size: JIRSize, i8),
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);
        match input {
            JIRValue::Slice(_) => {
                let input_reg = reg!(self.allocations, inst, 1);
                // Copy the input into the output.
                // The x86 shl operation operates in place.
                self.copy_reg(output_reg, input_reg, input.size(), CopyExtendMode::None)?;
            }
            JIRValue::Const(input) => {
                // We load the const directly into output
                self.load_const(output_reg, input)?;
            }
        };
        // TODO: We can const optimize the (const, const) case to a move.
        match shift {
            JIRValue::Slice(shift) => {
                asm_reg(self.block.assembler(), output_reg, output.size());
            }
            JIRValue::Const(shift) => {
                // We simply cut off the shift amount over 8 bits because it would be meaningless to shift more
                // This is only the case for slices that are <= 8 bytes.
                let shift = shift.value() as i8;
                asm_imm(self.block.assembler(), output_reg, output.size(), shift);
            }
        }
        Ok(())
    }
}
