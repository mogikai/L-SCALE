use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::assembler::MogiAssembler;
use crate::jit::trace_jit;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::x64emitter::X64Emitter;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};

impl X64Emitter<'_> {
    pub fn emit_shift_left(
        &mut self,
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
            output,
            input,
            shift,
            |asm, out_gpr, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_gpr as u8), cl
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_gpr as u8), cl
                ),
                _ => todo!(),
            },
            |asm, out_gpr, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shl Rd(out_gpr as u8), imm
                ),
                ..=8 => assemble!(asm;
                    ; shl Rq(out_gpr as u8), imm
                ),
                _ => todo!(),
            },
        )
    }

    pub fn emit_shift_right(
        &mut self,
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
            output,
            input,
            shift,
            |asm, out_gpr, size| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_gpr as u8), cl
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_gpr as u8), cl
                ),
                _ => todo!(),
            },
            |asm, out_gpr, size, imm| match size {
                ..=4 => assemble!(asm;
                    // Use the smaller op encoding if possible
                    ; shr Rd(out_gpr as u8), imm
                ),
                ..=8 => assemble!(asm;
                    ; shr Rq(out_gpr as u8), imm
                ),
                _ => todo!(),
            },
        )
    }

    pub fn emit_shift_right_signed(
        &mut self,
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
            output,
            input,
            shift,
            |asm, out_gpr, size| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_gpr as u8), cl
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_gpr as u8), cl
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_gpr as u8), cl
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_gpr as u8), cl
                ),
                _ => todo!("Unsupported shift size"),
            },
            |asm, out_gpr, size, imm| match size {
                1 => assemble!(asm;
                    ; sar Rb(out_gpr as u8), imm
                ),
                2 => assemble!(asm;
                    ; sar Rw(out_gpr as u8), imm
                ),
                4 => assemble!(asm;
                    ; sar Rd(out_gpr as u8), imm
                ),
                8 => assemble!(asm;
                    ; sar Rq(out_gpr as u8), imm
                ),
                _ => todo!("Unsupported shift size"),
            },
        )
    }

    /// This function emits the setup code for a shift operation, i.e. loading registers.
    /// Then it calls one of the callbacks to emit the actual shift instruction.
    fn shift(
        &mut self,
        output: JIRSlice,
        input: JIRValue,
        shift: JIRValue,
        asm_reg: fn(&mut MogiAssembler<X64Relocation>, GPRegister, size: JIRSize),
        asm_imm: fn(&mut MogiAssembler<X64Relocation>, GPRegister, size: JIRSize, i8),
    ) -> JITResult {
        match shift {
            JIRValue::Slice(shift) => {
                // shl expects its argument in the CL register.
                self.load_slice_gpr_exact(GPRegister::CX, shift, SliceType::Input)?;
                // We want to directly load the input into the output and then shift the output.
                // The x86 shl operation operates in place.
                self.emit_copy_deprecated(output, input)?;
                let out_gpr = self.load_slice_gpr(output, SliceType::Output);
                asm_reg(self.block.assembler(), out_gpr, output.size());
            }
            JIRValue::Const(shift) => {
                // We simply cut off the shift amount over 8 bits because it would be meaningless to shift more
                // This is only the case for slices that are <= 8 bytes.
                let shift = shift.value() as i8;
                // We want to directly load the input into the output and then shift the output.
                // The x86 shl operation operates in place.
                self.emit_copy_deprecated(output, input)?;
                let out_gpr = self.load_slice_gpr(output, SliceType::Output);
                asm_imm(self.block.assembler(), out_gpr, output.size(), shift);
            }
        }
        Ok(())
    }
}
