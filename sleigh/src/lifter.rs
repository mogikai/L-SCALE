use crate::compiler::CompilerContext;
use crate::lifter::cache::LifterCache;
use crate::spec::ProcessorSpecification;
use crate::spec::builder::CustomInterner;
use crate::spec::jir::context_provider::ContextProvider;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::local_resolver::JIRResolver;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::context::MOIRContextId;
use crate::spec::moir::table::TableId;
use mogi_utilities::addresses::EVAddr;
use source::TargetInstructionSource;
use thiserror::Error;
use tracing::debug;

pub mod cache;
pub mod source;

pub type LifterResult<T = ()> = Result<T, LifterError>;

#[derive(Debug, Error, Copy, Clone)]
pub enum LifterError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The address {0:?} is already associated with a cached instruction")]
    InstructionAlreadyCached(EVAddr),
    #[error("The address {0:?} is already associated with a cached block")]
    BlockAlreadyCached(EVAddr),
    #[error("The cache has no entry for the requested address {0:?}")]
    MissingCacheEntry(EVAddr),
}

pub struct Lifter<'a, 'cache, InstSourceT: TargetInstructionSource> {
    specification: &'a ProcessorSpecification,
    cache: &'cache mut LifterCache,
    instruction_source: &'a InstSourceT,
}

impl<'a, 'cache: 'a, InstSourceT: TargetInstructionSource> Lifter<'a, 'cache, InstSourceT> {
    pub fn new(
        specification: &'a ProcessorSpecification,
        cache: &'cache mut LifterCache,
        instruction_source: &'a InstSourceT,
    ) -> Self {
        Self {
            specification,
            cache,
            instruction_source,
        }
    }
    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted instruction
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetInstruction] for more detail.
    pub fn lift<'slf>(
        &mut self,
        addr: EVAddr,
        cid: usize,
    ) -> LifterResult<(EVAddr, &TargetInstruction)> {
        let root_jumplist = self.specification.jumplist_cache.get(0.into()).unwrap();

        // mod=1
        let bytes = [0x48, 0x05, 0x34, 0x12, 0x00, 0x00];

        let mut context_provider = ContextProvider::new(&self.specification.symbol_tables.fields);
        // longMode=1
        // addrSize=2
        *context_provider
            .context_value_mut(MOIRContextId::from(0))
            .unwrap() = 0x21;

        // TODO finish matching
        let resolver = root_jumplist
            .r#match(
                &bytes,
                &self.specification.jumplist_cache,
                &self.specification,
                &self.specification.symbol_tables.constructors,
                &mut context_provider,
            )
            .unwrap();

        if let Some((_, resolver)) = resolver {
            resolver.print_hierarchy(0);
            resolver.print_mcode(0);

            let target_instruction = resolver.build().unwrap();
            trace_jir!("{target_instruction}");
        }

        todo!("return values")
    }

    fn lift_block(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
    ) -> LifterResult<(EVAddr, Vec<TargetInstruction>)> {
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
