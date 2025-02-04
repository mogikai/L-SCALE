use std::hint::black_box;
use std::time::Instant;

use crate::lifter::cache::LifterCache;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::basic_block::{
    TargetBasicBlock,
    TargetBasicBlockBuilder,
};
use crate::spec::jir::context_provider::ContextProvider;
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
    context_provider: &'a mut ContextProvider,
    cache: &'cache mut LifterCache,
    instruction_source: &'a InstSourceT,
}

impl<'a, 'cache, InstSourceT: TargetInstructionSource> Lifter<'a, 'cache, InstSourceT> {
    pub fn new(
        specification: &'a ProcessorSpecification,
        context_provider: &'a mut ContextProvider,
        cache: &'cache mut LifterCache,
        instruction_source: &'a InstSourceT,
    ) -> Self {
        Self {
            specification,
            context_provider,
            cache,
            instruction_source,
        }
    }
    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted basic block,
    /// containing only this single instruction.
    /// To decode the entire target basic block, use [Self::lift_block] instead.
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetBasicBlock] for more detail.
    pub fn lift_single(
        &mut self,
        addr: EVAddr,
    ) -> LifterResult<(EVAddr, &TargetBasicBlock)> {
        self.cache.flush_block(addr)?;
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

        let bytes = self
            .instruction_source
            .load(addr)
            .map_err(|_| -> LifterError { todo!() })?;

        let root_jumplist = self.specification.jumplist_cache.get(0.into()).unwrap();

        let resolver = root_jumplist
            .r#match(
                &bytes,
                &self.specification.jumplist_cache,
                &self.specification.symbol_tables.constructors,
                &mut self.context_provider,
            )
            .unwrap();

        if let Some((_, resolver)) = resolver {
            let mut block_builder = TargetBasicBlockBuilder::new(addr, self.specification.temp_space);
            // resolver.print_hierarchy(0);
            // resolver.print_mcode(&self.specification, 0);

            resolver
                .build(&self.specification, &mut block_builder)
                .map_err(|_| -> LifterError { todo!() })?;

            let block = block_builder
                .finish()
                .map_err(|_| -> LifterError { todo!() })?;
            let block = self.cache.cache_block(addr, block)?;
            Ok((addr, block))
        } else {
            Err(LifterError::Unknown)
        }
    }

    fn lift_block(
        &self,
        _addr: EVAddr,
        _source: impl TargetInstructionSource,
    ) -> LifterResult<(EVAddr, &TargetBasicBlock)> {
        todo!()
    }

    // TODO: Move this to disassembler.
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
