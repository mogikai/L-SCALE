use crate::jit::api::JITResult;
use crate::jit::x86::X86MogiJit;
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X86MogiJit {
    pub fn emit_load(
        &mut self,
        output: JIRSlice,
        addr: JIRValue,
        space_id: SpaceId,
    ) -> JITResult {
        todo!()
    }

    pub fn emit_store(
        &mut self,
        addr: JIRValue,
        space_id: SpaceId,
        value: JIRValue,
    ) -> JITResult {
        todo!()
    }
}
