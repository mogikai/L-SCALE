use crate::jit::api::JITResult;
use crate::jit::code_buffer::CompiledBasicBlock;
use crate::jit::regalloc::requirements::OperandRequest;
use crate::jit::x86::x64emitter::arithmetic::integer::AddSub;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::{
    X64Emitter,
    mov64,
};
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use dynasmrt::x64::X64Relocation;
use either::Either;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::iter::once;
use tracing::trace;

impl X64Emitter<'_> {
    pub fn request_add_operands(
        &self,
        output: JIRSlice,
        a: JIRValue,
        b: JIRValue,
    ) -> impl Iterator<Item = OperandRequest> {
        once(OperandRequest::write_any(output))
    }

    /// Emit instructions that add `a` and `b` and store the result in `output`.
    /// Since JIR operations do not have side effects, most of the time this will
    /// result in some kind of lea operation.
    #[inline]
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
                self.emit_copy(output, JIRConst::new((a + b) % (1u128 << (size * 8)), size))?;
            }
            // Addition is commutative, so we can simply swap a and b if one is const and the other is a varnode.
            (JIRValue::Const(b), JIRValue::Slice(a)) | (JIRValue::Slice(a), JIRValue::Const(b)) => {
                self.emit_add_reg_const(output, a, b.value())?;
            }
            (JIRValue::Slice(a), JIRValue::Slice(b)) => {
                let a: u8 = self.load_slice_gpr(a, SliceType::Input).into();
                let b: u8 = self.load_slice_gpr(b, SliceType::Input).into();
                let output: u8 = self.load_slice_gpr(output, SliceType::Output).into();
                assemble!(self.block.assembler();
                    ; lea Rq(output), [Rq(a) + Rq(b)]
                );
            }
        }
        Ok(())
    }

    #[inline]
    fn emit_add_reg_const(
        &mut self,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
    ) -> JITResult {
        self.emit_add_sub_reg_const(output, a, b, AddSub::Add)
    }

    /// Emit instructions to add or subtract the provided constant value to the register `a`.
    /// Subtraction is done by adding the inverse element of the constant.
    #[inline]
    pub(super) fn emit_add_sub_reg_const(
        &mut self,
        output: JIRSlice,
        a: JIRSlice,
        b: u128,
        add_sub: AddSub,
    ) -> JITResult {
        let size = output.size();
        let a: u8 = self.load_slice_gpr(a, SliceType::Input).into();
        let output: u8 = self.load_slice_gpr(output, SliceType::Output).into();
        if b < i8::MAX as u128 {
            // Use byte sized displacement for operands that fit into one byte.
            let mut b = b as i8;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(self.block.assembler();
                ; lea Rq(output), [BYTE Rq(a) + b]
            );
        } else if b < i32::MAX as u128 {
            // Use the normal 32 bit displacement for anything that fits into an i32
            let mut b = b as i32;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            assemble!(self.block.assembler();
                ; lea Rq(output), [Rq(a) + b]
            );
        } else if b < u64::MAX as u128 && size <= 8 {
            // If b is > i32::MAX, it is not possible to encode the operand as displacement for
            // a lea instruction, since the displacement can be at most 32 bits.

            // Use mov to load the 64 bit immediate into a temp register, since it is
            // the only instruction that supports a 64 bit immediate value.
            let mut b = b as i128;
            if let AddSub::Sub = add_sub {
                b = -b;
            }
            let tmp = self.get_temp_gpr();
            assemble!(self.block.assembler();
                ;; mov64!(self.block.assembler(); output, a)
                ; mov Rq(tmp as u8), QWORD b as _
                ; add Rq(output), Rq(tmp as u8)
            );
        } else {
            todo!("Immediates > u64::MAX currently unsupported")
        }
        Ok(())
    }
}
