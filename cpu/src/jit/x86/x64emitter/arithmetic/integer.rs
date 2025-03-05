mod add;
mod divide;
mod multiply;
mod negate;
mod shift;
mod subtract;
#[cfg(test)]
mod test;

use crate::jit::api::JITResult;
use crate::jit::x86::x64emitter::X64Emitter;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

enum AddSub {
    Add,
    Sub,
}

impl X64Emitter<'_> {
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
