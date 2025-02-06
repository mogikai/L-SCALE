use crate::lifter::cache::LifterCache;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::basic_block::{
    TargetBasicBlock,
    TargetBasicBlockBuilder,
};
use crate::spec::jir::context_provider::ContextProvider;
use mogi_utilities::addresses::EVAddr;
use source::TargetInstructionSource;
use std::time::{
    Duration,
    Instant,
};
use thiserror::Error;

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

pub struct Lifter<'a, InstSourceT: TargetInstructionSource> {
    context_provider: &'a mut ContextProvider,
    cache: LifterCache,
    instruction_source: &'a InstSourceT,
    diagnostics: LifterDiagnostics,
}

#[derive(Debug, Default)]
pub struct LifterDiagnostics {
    total_time_match: Duration,
    total_time_build: Duration,
    inst_lifted_since_last_reset: u32,
}

impl LifterDiagnostics {
    pub fn match_avg(&self) -> Duration {
        self.total_time_match / self.inst_lifted_since_last_reset
    }

    pub fn build_avg(&self) -> Duration {
        self.total_time_build / self.inst_lifted_since_last_reset
    }
}

impl<'a, InstSourceT: TargetInstructionSource> Lifter<'a, InstSourceT> {
    pub fn new(
        context_provider: &'a mut ContextProvider,
        cache: LifterCache,
        instruction_source: &'a InstSourceT,
    ) -> Self {
        Self {
            context_provider,
            cache,
            instruction_source,
            diagnostics: LifterDiagnostics::default(),
        }
    }

    pub fn diagnostics(&self) -> &LifterDiagnostics {
        &self.diagnostics
    }

    pub fn diagnostics_mut(&mut self) -> &mut LifterDiagnostics {
        &mut self.diagnostics
    }

    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted basic block,
    /// containing only this single instruction.
    /// To decode the entire target basic block, use [Self::lift_block] instead.
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetBasicBlock] for more detail.
    pub fn lift_single(
        &mut self,
        specification: &ProcessorSpecification,
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
        self.diagnostics.inst_lifted_since_last_reset += 1;

        let bytes = self
            .instruction_source
            .load(addr)
            .map_err(|_| -> LifterError { todo!() })?;

        let now = Instant::now();
        let root_jumplist = specification.jumplist_cache.get(0.into()).unwrap();

        let resolver = root_jumplist
            .r#match(
                &bytes,
                &specification.jumplist_cache,
                &specification.symbol_tables.constructors,
                &mut self.context_provider,
            )
            .unwrap();
        self.diagnostics.total_time_match += now.elapsed();

        let now = Instant::now();
        if let Some((_, resolver)) = resolver {
            let mut block_builder = TargetBasicBlockBuilder::new(addr, specification.temp_space);
            // resolver.print_hierarchy(0);
            // resolver.print_mcode(&self.specification, 0);

            resolver
                .build(specification, &mut block_builder)
                .map_err(|_| -> LifterError { todo!() })?;

            let block = block_builder
                .finish()
                .map_err(|_| -> LifterError { todo!() })?;
            let block = self.cache.cache_block(addr, block)?;
            self.diagnostics.total_time_build += now.elapsed();
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
