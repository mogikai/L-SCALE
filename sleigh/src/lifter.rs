use std::time::Instant;

use crate::compiler::CompilerContext;
use crate::lifter::cache::LifterCache;
use crate::spec::ProcessorSpecification;
use crate::spec::builder::CustomInterner;
use crate::spec::jir::context_provider::ContextProvider;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::local_resolver::JIRResolver;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
    VarNodeId,
};
use crate::spec::moir::address_space::SpaceId;
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
        // self.cache.flush_instruction(addr)?;
        // // Check the cache. If it contains the instruction we can simply return it.
        // if self.cache.is_instruction_cached(addr) {
        //     // TODO: Remove the outer check once the conditional control flow borrow checking lands
        //     //       See https://github.com/rust-lang/rfcs/blob/master/text/2094-nll.md#problem-case-3-conditional-control-flow-across-functions
        //     match self.cache.fetch_instruction(addr) {
        //         Ok(instruction) => {
        //             return Ok((instruction.next(), instruction));
        //         }
        //         Err(_) => unreachable!(),
        //     }
        // }
        // // No cache entry? Fetch data now.
        // let data = self.instruction_source.load(addr);
        // // TODO: Perform the matching on the instruction bytes to
        // //       get the correct constructor and resolve context

        let root_jumplist = self.specification.jumplist_cache.get(0.into()).unwrap();

        // mod=1
        let bytes = [ 0xC4, 0xE2, 0xE3, 0xF5, 0x04, 0xAD, 0x37, 0x13, 0x00, 0x00 ];

        let mut context_provider = ContextProvider::new(&self.specification.symbol_tables.fields);
        // longMode=1
        // addrSize=2
        *context_provider
            .context_value_mut(MOIRContextId::from(0))
            .unwrap() = 0x21;

        let count = 1_000;
        let start = Instant::now();
        for _ in 0..count {
            let resolver = root_jumplist
                .r#match(
                    &bytes,
                    &self.specification.jumplist_cache,
                    &self.specification,
                    &self.specification.symbol_tables.constructors,
                    &mut context_provider,
                )
                .unwrap();

            std::hint::black_box(resolver);
        }

        let duration = (Instant::now() - start) / count;
        eprintln!("duration = {:?}", duration);

        // if let Some((_, resolver)) = resolver {
        //     resolver.print_hierarchy(0);
        //     resolver.print_mcode(0);

        //     let target_instruction = resolver.build().unwrap();
        //     trace_jir!("{target_instruction}");
        // }

        todo!("return values")
    }

    fn lift_block(
        &self,
        _addr: EVAddr,
        _source: impl TargetInstructionSource,
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
