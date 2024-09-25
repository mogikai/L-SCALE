use crate::compiler::CompilerContext;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::lifter::{
    JIRLifter,
    JIRLifterResult,
};
use crate::spec::jir::local_resolver::JIRLocalResolver;
use crate::spec::jir::source::TargetInstructionSource;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use crate::spec::moir::constructor::ConstructorId;
use mogi_utilities::addresses::EVAddr;
use tracing::debug;

impl JIRLifter for ProcessorSpecification {
    fn lift(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
        cid: usize,
    ) -> JIRLifterResult<(EVAddr, TargetInstruction)> {
        let data = source.load(addr);
        let constructor = self
            .symbol_tables
            .constructors
            .get(ConstructorId::from(cid))
            .unwrap();
        debug!("{}", constructor.mcode_debug_string(&self.interner));
        let mut resolver = JIRLocalResolver::default();
        resolver
            .define_ident(
                &constructor.local_scope,
                self.interner.lookup("Reg32").unwrap(),
                JIRValue::Slice(JIRSlice {
                    varnode_id: VarNodeId::from(0),
                    start: 0,
                    end: 4,
                }),
            )
            .unwrap();
        constructor.generate_jcode(resolver).unwrap();
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
