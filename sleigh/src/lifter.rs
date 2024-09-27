use crate::compiler::CompilerContext;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::local_resolver::JIRLocalResolver;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use crate::spec::moir::constructor::ConstructorId;
use mogi_utilities::addresses::EVAddr;
use source::TargetInstructionSource;
use thiserror::Error;
use tracing::debug;

pub mod cache;
pub mod source;

pub type JIRLifterResult<T = ()> = Result<T, JIRLifterError>;

#[derive(Debug, Error)]
pub enum JIRLifterError {
    #[error("An unknown error occurred")]
    Unknown,
}

pub struct JIRLifter<'a, InstSourceT: TargetInstructionSource> {
    specification: &'a ProcessorSpecification,
    instruction_source: &'a InstSourceT,
}

impl<'a, InstSourceT: TargetInstructionSource> JIRLifter<'a, InstSourceT> {
    pub fn new(
        specification: &'a ProcessorSpecification,
        instruction_source: &'a InstSourceT,
    ) -> Self {
        Self {
            specification,
            instruction_source,
        }
    }
    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted instruction
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetInstruction] for more detail.
    pub fn lift(
        &self,
        addr: EVAddr,
        cid: usize,
    ) -> JIRLifterResult<(EVAddr, TargetInstruction)> {
        let data = self.instruction_source.load(addr);
        let constructor = self
            .specification
            .symbol_tables
            .constructors
            .get(ConstructorId::from(cid))
            .expect("");
        debug!("{}", constructor.mcode_debug_string(&self.specification.interner));
        // TODO: Calculate size of instruction and add it here.
        // TODO: For inst_next2 we need to decode the next instruction as well.
        //       since this is expensive, we probably should buffer this, and
        //       also only calculate inst_next2 if we actually need it in the body.
        //       For now we just set it to None
        let inst_next = addr + 8;
        let mut resolver = JIRLocalResolver::new(addr, inst_next, None);
        resolver
            .define_ident(
                &constructor.local_scope,
                self.specification.interner.lookup("Reg32").unwrap(),
                JIRValue::Slice(JIRSlice {
                    varnode_id: VarNodeId::from(0),
                    start: 0,
                    end: 4,
                }),
            )
            .expect("");
        resolver
            .define_ident(
                &constructor.local_scope,
                self.specification.interner.lookup("vexVVVV_r32").unwrap(),
                JIRValue::Slice(JIRSlice {
                    varnode_id: VarNodeId::from(0),
                    start: 0,
                    end: 4,
                }),
            )
            .expect("");
        resolver
            .define_ident(
                &constructor.local_scope,
                self.specification.interner.lookup("rm32").unwrap(),
                JIRValue::Slice(JIRSlice {
                    varnode_id: VarNodeId::from(0),
                    start: 0,
                    end: 4,
                }),
            )
            .expect("");
        constructor.generate_jcode(resolver).expect("");
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

    // fn disasm(
    //     &self,
    //     data: &[u8],
    // ) -> JIRLifterResult<(usize, String)> {
    //     todo!()
    // }
    //
    // fn disasm_many(
    //     &self,
    //     data: &[u8],
    // ) -> JIRLifterResult<(usize, Vec<String>)> {
    //     todo!()
    // }
}
