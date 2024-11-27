use crate::jit::api::JitError;
use crate::jit::x86::emit::mov64;
use crate::jit::x86::{
    assemble,
    X86MogiJit,
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
    ) -> Result<(), JitError> {
        debug_assert_eq!(output.size(), a.size());
        debug_assert_eq!(a.size(), b.size());
        match (a, b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                trace!("Const add optimized to move");
                let a = a.value();
                let b = b.value();
                let size = output.size();
                self.emit_copy(output, JIRConst::new((a + b) % (size * 8) as u128, size).into())?;
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
    ) -> Result<(), JitError> {
        self.emit_add_sub_reg_const(output, a, b, AddSub::Add)
    }

    /// Emit instructions to add the provided constant value to the register `a`.
    fn emit_add_sub_reg_const(
        &mut self,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
        add_sub: AddSub,
    ) -> Result<(), JitError> {
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
        a: JIRValue,
        b: JIRValue,
    ) -> Result<(), JitError> {
        debug_assert_eq!(output.size(), a.size());
        debug_assert_eq!(a.size(), b.size());
        let size = output.size();
        match (a, b) {
            (JIRValue::Const(a), JIRValue::Const(b)) => {
                trace!("Const subtract optimized to move");
                let a = a.value();
                let b = b.value();
                let size = output.size();
                self.emit_copy(output, JIRConst::new((a - b) % (size * 8) as u128, size).into())?;
            }
            (JIRValue::Slice(a), JIRValue::Const(b)) => {
                self.emit_add_sub_reg_const(output, a, b.value(), AddSub::Sub)?;
            }
            (JIRValue::Const(a), JIRValue::Slice(b)) => {
                todo!()
            }
            _ => {
                todo!()
            }
        }
        Ok(())
    }
}
