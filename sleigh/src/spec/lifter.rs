use crate::spec::jir::code::JIRInstruction;
use crate::spec::jir::lifter::{
    JIRLifter,
    JIRLifterResult,
};
use crate::spec::ProcessorSpecification;

impl JIRLifter for ProcessorSpecification {
    fn lift(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, Vec<JIRInstruction>)> {
        // TODO: Get root table and start matching.
        // TODO: It probably makes sense to do this in a
        // TODO: separate function in order to recursively call the matching?
        todo!()
    }

    fn lift_many(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, ())> {
        todo!()
    }

    fn disasm(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, String)> {
        todo!()
    }

    fn disasm_many(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, Vec<String>)> {
        todo!()
    }
}
