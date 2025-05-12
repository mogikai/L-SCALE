use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    trace_emit, X64AllocRequirements, X64Emitter
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
        debug_assert_eq!(output.size(), input.size());

        match (input, shift) {
            (JIRValue::Const(_), JIRValue::Const(_)) => {
                // TODO: delegate to copy?
                Self::require_large_int_write_any(requirement, output);
            }
            (JIRValue::Const(_), JIRValue::Slice(shift)) => {
                // Shift is always loaded in cx on x86
                Self::require_large_int_write_any(requirement, output);
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::CX.into_host_reg(), shift));
            }
            (JIRValue::Slice(input), JIRValue::Slice(shift)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input);
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::CX.into_host_reg(), shift));
            }
            (JIRValue::Slice(input), JIRValue::Const(_)) => {
                Self::require_large_int_write_any(requirement, output);
                Self::require_large_int_read_any(requirement, input);
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
        trace_emit!("Emitting shl {output:?} = {input:?} << {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, scratch, out_reg_lower, out_reg_upper, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_reg_lower.hw_enc()), cl
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_reg_lower.hw_enc()), cl
                ),
                16 => assemble!(asm;
                    // Shift the upper part stealing from lower
                    ; shld Rq(out_reg_upper.hw_enc()), Rq(out_reg_lower.hw_enc()), cl
                    // Shift the lower part. Shift amount is masked, such that shifts are mod 64 bits
                    // i.e. shifting 65 bits is equivalent to shifting 1 bit.
                    ; shl Rq(out_reg_lower.hw_enc()), cl
                    ; xor Rq(scratch.hw_enc()), Rq(scratch.hw_enc())
                    // Check if shift > 64. This is the case when bit 6 is set.
                    ; test cl, 0b1_000_000
                    // If bit 6 is set and thus shift > 64 we want to copy lower into upper.
                    ; cmovnz Rq(out_reg_upper.hw_enc()), Rq(out_reg_lower.hw_enc())
                    // And then also zero lower.
                    ; cmovnz Rq(out_reg_lower.hw_enc()), Rq(scratch.hw_enc())
                ),
                _ => todo!(),
            },
            |asm, scratch, out_reg_lower, out_reg_upper, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_reg_lower.hw_enc()), imm
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_reg_lower.hw_enc()), imm
                ),
                16 if imm < 64 => assemble!(asm;
                    ; shld Rq(out_reg_upper.hw_enc()), Rq(out_reg_lower.hw_enc()), imm
                    ; shl Rq(out_reg_lower.hw_enc()), imm
                ),
                16 if imm >= 64 => assemble!(asm;
                    ; mov Rq(out_reg_upper.hw_enc()), Rq(out_reg_lower.hw_enc())
                    ; shl Rq(out_reg_upper.hw_enc()), imm % 64 // Shift modulo 64 because we already performed 64 bit shift by moving lower into upper
                    ; xor Rq(out_reg_lower.hw_enc()), Rq(out_reg_lower.hw_enc())
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
        if output.size() > 16 {
            todo!("Sizes greater than 8 are currently unsupported for shift operations.")
        }
        trace_emit!("Emitting shr {output:?} = {input:?} >> {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, scratch, out_reg_lower, out_reg_upper, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_reg_lower.hw_enc()), cl
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_reg_lower.hw_enc()), cl
                ),
                16 => assemble!(asm;
                    // Shift the lower part stealing from upper
                    ; shrd Rq(out_reg_lower.hw_enc()), Rq(out_reg_upper.hw_enc()), cl
                    // Shift the upper part. Shift amount is masked, such that shifts are mod 64 bits
                    // i.e. shifting 65 bits is equivalent to shifting 1 bit.
                    ; shr Rq(out_reg_upper.hw_enc()), cl
                    ; xor Rq(scratch.hw_enc()), Rq(scratch.hw_enc())
                    // Check if shift > 64. This is the case when bit 6 is set.
                    ; test cl, 0b1_000_000
                    // If bit 6 is set and thus shift > 64 we want to copy upper into lower
                    ; cmovnz Rq(out_reg_lower.hw_enc()), Rq(out_reg_upper.hw_enc())
                    // And then also zero upper.
                    ; cmovnz Rq(out_reg_upper.hw_enc()), Rq(scratch.hw_enc())
                ),
                _ => todo!(),
            },
            |asm, scratch, out_reg_lower, out_reg_upper, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_reg_lower.hw_enc()), imm
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_reg_lower.hw_enc()), imm
                ),
                16 if imm < 64 => assemble!(asm;
                    ; shrd Rq(out_reg_lower.hw_enc()), Rq(out_reg_upper.hw_enc()), imm
                    ; shr Rq(out_reg_upper.hw_enc()), imm
                ),
                16 if imm >= 64 => assemble!(asm;
                    ; mov Rq(out_reg_lower.hw_enc()), Rq(out_reg_upper.hw_enc())
                    ; shr Rq(out_reg_lower.hw_enc()), imm % 64 // Shift modulo 64 because we already performed 64 bit shift by moving lower into upper
                    ; xor Rq(out_reg_upper.hw_enc()), Rq(out_reg_upper.hw_enc())
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
        trace_emit!("Emitting shr {output:?} = {input:?} s>> {shift:?}");

        self.shift(
            inst,
            output,
            input,
            shift,
            |asm, scratch, out_reg_lower, out_reg_upper, size| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_reg_lower.hw_enc()), cl
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_reg_lower.hw_enc()), cl
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_reg_lower.hw_enc()), cl
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_reg_lower.hw_enc()), cl
                ),
                _ => todo!("Unsupported shift size"),
            },
            |asm, scratch, out_reg_lower, out_reg_upper, size, imm| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_reg_lower.hw_enc()), imm
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_reg_lower.hw_enc()), imm
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_reg_lower.hw_enc()), imm
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_reg_lower.hw_enc()), imm
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
        asm_reg: fn(&mut MogiAssembler<X64Relocation>, HostReg, HostReg, HostReg, size: JIRSize),
        asm_imm: fn(&mut MogiAssembler<X64Relocation>, HostReg, HostReg, HostReg, size: JIRSize, i8),
    ) -> JITResult {
        let output_reg_lower = reg!(self.allocations, inst, 0);
        let output_reg_upper = if output.size() == 16 {
            reg!(self.allocations, inst, 1)
        } else {
            HostReg::invalid()
        };
        match input {
            JIRValue::Slice(_) => {
                // Copy the input into the output.
                // The x86 shl operation operates in place.
                if output.size() <= 8 {
                    let input_reg_lower = reg!(self.allocations, inst, 1);
                    self.copy_reg(output_reg_lower, input_reg_lower, input.size(), CopyExtendMode::None)?;
                } else if output.size() == 16 {
                    let input_reg_lower = reg!(self.allocations, inst, 2);
                    let input_reg_upper = reg!(self.allocations, inst, 3);
                    self.copy_reg(output_reg_lower, input_reg_lower, 8, CopyExtendMode::None)?;
                    self.copy_reg(output_reg_upper, input_reg_upper, 8, CopyExtendMode::None)?;
                } else {
                    todo!("Invalid size")
                }
            }
            JIRValue::Const(input) => {
                // We load the const directly into output
                if output.size() <= 8 {
                    self.load_const(output_reg_lower, input)?;
                } else if output.size() == 16 {
                    let (input_lower, input_upper) = input.split(8);
                    self.load_const(output_reg_lower, input_lower)?;
                    self.load_const(output_reg_upper, input_upper)?;
                } else {
                    todo!("Invalid size")
                }
            }
        };
        // TODO: We can const optimize the (const, const) case to a move.
        match shift {
            JIRValue::Slice(shift) => {
                // Shift is already allocated to cl by the register allocator.
                asm_reg(
                    self.block.assembler(),
                    self.allocations.scratch_register(RegClass::Int),
                    output_reg_lower,
                    output_reg_upper,
                    output.size(),
                );
            }
            JIRValue::Const(shift) => {
                // We simply cut off the shift amount over 8 bits because it would be meaningless to shift more
                // This is only the case for slices that are <= 8 bytes.
                let shift = shift.value() as i8;
                asm_imm(
                    self.block.assembler(),
                    self.allocations.scratch_register(RegClass::Int),
                    output_reg_lower,
                    output_reg_upper,
                    output.size(),
                    shift,
                );
            }
        }
        Ok(())
    }
}
