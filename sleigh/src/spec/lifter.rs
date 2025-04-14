use crate::lifter::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::lifter::{
    Lifter,
    LifterDiagnostics,
    LifterError,
    LifterResult,
};
use crate::spec::ProcessorSpecification;
use crate::spec::jir::basic_block::TargetBasicBlock;
use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
use crate::spec::jir::context_stack::ContextStack;
use crate::spec::jir::jcode::ContinueLifting;
use crate::spec::jir::jumplist::MatcherResult;
use crate::spec::jir::trace_jir;
use mogi_utilities::addresses::EVAddr;
use std::cell::Ref;
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::time::Instant;

use super::jir::common::JIRSize;
use super::jir::jumplist::MatchContext;

impl Lifter for ProcessorSpecification {
    fn lift_block<S, C>(
        &self,
        source: &S,
        context_provider: &mut C,
        mut addr: EVAddr,
        max_instructions: Option<NonZeroUsize>,
    ) -> LifterResult<TargetBasicBlock>
    where
        S: TargetInstructionSource,
        C: ContextProvider,
    {
        // TODO: Allocate buffer size depending on the target instruction set's maximum instruction size.
        let mut buffer = [0u8; 16];

        let now = Instant::now();
        let mut block_builder = TargetBasicBlockBuilder::new(addr, self.temp_space);
        let mut block_size = 0;
        // Start with a block. Any intermediate blocks will end this block and create a new one.
        block_builder.start_block().unwrap();
        let mut instruction_count = 0usize;
        loop {
            trace_jir!("Loading instruction bytes at {:?}", addr);
            source
                .load(addr, &mut buffer)
                .map_err(|_| -> LifterError { LifterError::InputLoad(addr) })?;

            // TODO error type
            // TODO: Initially fill the context stack with the value from the context provider that needs to be passed in.

            let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
            let match_context = MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors);
            let matching_result = match_context.r#match(&buffer, &mut context_stack).unwrap();
            self.lifter_diagnostics.borrow_mut().total_time_match += now.elapsed();

            let now = Instant::now();
            #[allow(unused_variables)]
            match matching_result {
                MatcherResult::EndOfByteStream => Err(LifterError::EndOfBytes)?,
                MatcherResult::None => Err(LifterError::NoMatch(buffer.to_vec()))?,
                MatcherResult::Some {
                    table: _,
                    resolver,
                    bytes_consumed,
                } => {
                    trace_jir!("Found match and consumed {} bytes", bytes_consumed);
                    trace_jir!("{}", resolver.hierarchy_debug_string().unwrap());

                    trace_jir!("{}", resolver.mcode_debug_string(&self).unwrap());
                    match resolver
                        .build(self, &mut block_builder)
                        .map_err(|e| -> LifterError { todo!("{e:?}") })?
                    {
                        ContinueLifting::Continue(next_addr) => addr = next_addr,
                        ContinueLifting::Stop => break,
                    }

                    block_size += bytes_consumed;
                    self.lifter_diagnostics.borrow_mut().total_time_build += now.elapsed();
                    instruction_count += 1;
                    if let Some(max_instructions) = max_instructions
                        && instruction_count >= max_instructions.into()
                    {
                        break;
                    }
                }
            }
        }

        // Close the last open block.
        block_builder.finish_block().unwrap();

        Ok(block_builder
            .finish()
            .map_err(|_| -> LifterError { todo!() })?)
    }

    fn diagnostics(&self) -> Ref<LifterDiagnostics> {
        self.lifter_diagnostics.borrow()
    }

    fn reset_diagnostics(&mut self) {
        *self.lifter_diagnostics.borrow_mut() = LifterDiagnostics::default();
    }
}
