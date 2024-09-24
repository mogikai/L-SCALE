use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::lifter::{
    JIRLifter,
    JIRLifterResult,
};
use crate::spec::jir::source::TargetInstructionSource;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::ProcessorSpecification;
use mogi_utilities::addresses::EVAddr;
use tracing::debug;

impl JIRLifter for ProcessorSpecification {
    fn lift(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
    ) -> JIRLifterResult<(EVAddr, TargetInstruction)> {
        let data = source.load(addr);
        let constructor = self
            .symbol_tables
            .constructors
            .get(ConstructorId::from(1143))
            .unwrap();
        debug!("{}", constructor.jcode_debug_string());
        // TODO: Get root table and start matching.
        // TODO: It probably makes sense to do this in a
        // TODO: separate function in order to recursively call the matching?
        Ok((addr, TargetInstruction::default()))
    }

    fn lift_block(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
    ) -> JIRLifterResult<(EVAddr, Vec<TargetInstruction>)> {
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
