use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::emit::mov64;
use crate::jit::x86::{
    GPRegister,
    X86MogiJit,
    assemble,
};
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use tracing::trace;

enum AddSub {
    Add,
    Sub,
}

impl X86MogiJit {
    /// Emit instructions that add `a` and `b` and store the result in `output`.
    /// Since JIR operations do not have side effects, most of the time this will
    /// result in some kind of lea operation.
    pub fn emit_add(
        &mut self,
        output: JIRSlice,
        a: JIRValue,
        b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), a.size());
        debug_assert_eq!(a.size(), b.size());
        match (a, b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                trace!("Const add optimized to move");
                let a = a.value();
                let b = b.value();
                let size = output.size();
                self.emit_copy(output, JIRConst::new((a + b) % (1u128 << (size * 8)), size).into())?;
            }
            // Addition is commutative, so we can simply swap a and b if one is const and the other is a varnode.
            (JIRValue::Const(b), JIRValue::Slice(a)) | (JIRValue::Slice(a), JIRValue::Const(b)) => {
                self.emit_add_reg_const(output, a, b.value())?;
            }
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                let a: u8 = self.load_slice_gp(a).into();
                let b: u8 = self.load_slice_gp(b).into();
                let output: u8 = self.load_slice_gp(output).into();
                let block = self.block_mut()?;
                assemble!(block.assembler;
                    ; lea Rq(output), [Rq(a) + Rq(b)]
                );
            }
        }
        Ok(())
    }

    fn emit_add_reg_const(
        &mut self,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
    ) -> JITResult {
        self.emit_add_sub_reg_const(output, a, b, AddSub::Add)
    }

    /// Emit instructions to add the provided constant value to the register `a`.
    fn emit_add_sub_reg_const(
        &mut self,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
        add_sub: AddSub,
    ) -> JITResult {
        let size = output.size();
        let a: u8 = self.load_slice_gp(a).into();
        let output: u8 = self.load_slice_gp(output).into();
        let block = self.block_mut()?;
        if b < i8::MAX as u128 {
            // Use byte sized displacement for operands that fit into one byte.
            let mut b = b as i8;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(block.assembler;
                ; lea Rq(output), [BYTE Rq(a) + b]
            );
        } else if b < i32::MAX as u128 {
            // Use the normal 32 bit displacement for anything that fits into an i32
            let mut b = b as i32;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(block.assembler;
                ; lea Rq(output), [Rq(a) + b]
            );
        } else if b < u64::MAX as u128 && size <= 8 {
            // If b is > i32::MAX, it is not possible to encode the operand as displacement for
            // a lea instruction, since the displacement can be at most 32 bits.

            // RAX is reserved for temporary use, so we can use it here.
            // Use mov to load the 64 bit immediate, since it is
            // the only instruction that supports a 64 bit immediate value.
            let mut b = b as i128;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(block.assembler;
                ;; mov64!(block.assembler; output, a)
                ; mov rax, QWORD b as _
                ; add Rq(output), rax
            );
        } else {
            todo!("Immediates > u64::MAX currently unsupported")
        }
        Ok(())
    }

    pub fn emit_subtract(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        debug_assert_eq!(output.size(), input_a.size());
        debug_assert_eq!(input_a.size(), input_b.size());
        let size = output.size();
        match (input_a, input_b) {
            (JIRValue::Const(input_a), JIRValue::Const(input_b)) => {
                trace!("Const subtract optimized to move");
                let size = output.size();
                self.emit_copy(
                    output,
                    JIRConst::new((input_a.value() - input_b.value()) % (size * 8) as u128, size).into(),
                )?;
            }
            (JIRValue::Slice(input_a), JIRValue::Const(input_b)) => {
                self.emit_add_sub_reg_const(output, input_a, input_b.value(), AddSub::Sub)?;
            }
            (JIRValue::Const(input_a), JIRValue::Slice(input_b)) => {
                self.emit_sub_const_reg(output, input_a, input_b)?;
            }
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                if input_a == input_b {
                    // A-A = 0, so we can just zero out the output.
                    self.emit_copy(output, JIRConst::new(0, size).into())?;
                } else if output == input_b {
                    let temp_gp = self.get_temp_gp();
                    // Move output=input_b to a temporary register.
                    self.emit_load_slice_to_gpr(temp_gp, output)?;
                    // Move input_a to output (possibly loading the slice if it is still unloaded)
                    self.emit_copy(output, input_a.into())?;
                    // Get the output gp register (it is loaded, since we just copied to it).
                    let output_gp = self.get_slice_gp(output);
                    // Subtract input_b=output ,which is stored in the temporary register, from the output (output=input_a).
                    assemble!(self.block_mut()?.assembler;
                        ; sub Rq(output_gp as u8), Rq(temp_gp as u8)
                    );
                } else {
                    // Copy input_a into output. If output == input_a we can just load output.
                    if output != input_a {
                        self.emit_copy(output, input_b.into())?;
                    };
                    let output_gp = self.load_slice_gp(output);
                    let input_b_gp = self.load_slice_gp(input_b);
                    // We can just subtract from output
                    assemble!(self.block_mut()?.assembler;
                        ; sub Rq(output_gp as u8), Rq(input_b_gp as u8)
                    );
                }
            }
        }
        Ok(())
    }

    /// Emit instructions to subtract the provided [JIRSlice] `input_b` from the provided [JIRConst] `input_a`.
    /// The result is stored in the [JIRSlice] `output`.
    fn emit_sub_const_reg(
        &mut self,
        output: JIRSlice,
        input_a: JIRConst,
        input_b: JIRSlice,
    ) -> JITResult {
        if output != input_b {
            // If the output register does not appear on the rhs, we can use it to load the constant
            // We use get_slice_gp here, since the value is overwritten anyway. No need to load here.
            let output_gp = self.get_slice_gp(output);
            let input_b_gp = self.load_slice_gp(input_b);
            self.emit_load_const_to_gpr(output_gp, input_a)?;
            // and then just subtract the operand register from the constant in the output register.
            assemble!(self.block_mut()?.assembler;
                ; sub Rq(output_gp as u8), Rq(input_b_gp as u8)
            );
        } else {
            let temp_gp = self.get_temp_gp();
            // Output appears on rhs (out = const - out)
            // Move the output into a temporary register
            self.emit_load_slice_to_gpr(temp_gp, output)?;
            // Load the constant into the output
            self.emit_copy(output, input_a.into())?;
            // Get the output gp register.
            let output_gp = self.get_slice_gp(output);
            // Subtract input_b=output ,which is stored in the temporary register, from the output (output=const).
            assemble!(self.block_mut()?.assembler;
                ; sub Rq(output_gp as u8), Rq(temp_gp as u8)
            );
        }
        Ok(())
    }

    pub fn emit_multiply(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        self.load_slice_gp(output);
        match (input_a, input_b) {
            (JIRValue::Slice(input_a), JIRValue::Slice(input_b)) => {
                let (target_gp, operand_gp) = self.load_commutative_inputs(output, input_a, input_b)?;
                assemble!(self.block_mut()?.assembler;
                    ; imul Rq(target_gp as u8), Rq(operand_gp as u8)
                );
            }
            (_, _) => {
                todo!()
            }
        }
        Ok(())
    }

    pub fn emit_divide(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        todo!()
    }

    pub fn emit_signed_divide(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        todo!()
    }

    pub fn emit_remainder(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        todo!()
    }

    pub fn emit_signed_remainder(
        &mut self,
        output: JIRSlice,
        input_a: JIRValue,
        input_b: JIRValue,
    ) -> JITResult {
        todo!()
    }
}
