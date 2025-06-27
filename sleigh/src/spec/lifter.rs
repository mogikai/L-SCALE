use crate::lifter::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::lifter::{
    BlockEndingOpDecider,
    Lifter,
    LifterConfig,
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

use super::jir::jumplist::MatchContext;

impl Lifter for ProcessorSpecification {
    fn lift_block<S, C, D>(
        &self,
        source: &S,
        context_provider: &mut C,
        config: &LifterConfig<D>,
        start_addr: EVAddr,
    ) -> LifterResult<TargetBasicBlock>
    where
        S: TargetInstructionSource,
        C: ContextProvider,
        D: BlockEndingOpDecider,
    {
        // TODO: Allocate buffer size depending on the target instruction set's maximum instruction size.
        let mut buffer = [0u8; 16];
        let mut addr = start_addr;

        let mut block_builder = TargetBasicBlockBuilder::new(addr, self.jir_groups.next_key());
        // Start with a block. Any intermediate blocks will end this block and create a new one.
        block_builder.start_block().unwrap();
        let mut instruction_count = 0usize;
        loop {
            if let Some(stop_addr) = config.stop_addr
                && stop_addr == addr
            {
                break;
            }

            trace_jir!("Loading instruction bytes at {:?}", addr);
            source
                .load(addr, &mut buffer)
                .map_err(|_| -> LifterError { LifterError::InputLoad(addr) })?;

            // TODO error type
            // TODO: Initially fill the context stack with the value from the context provider that needs to be passed in.

            let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
            let match_context = MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors);
            let matching_result = match_context.r#match(&buffer, &mut context_stack).unwrap();

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
                        .build(self, &mut block_builder, config)
                        .map_err(|e| -> LifterError { todo!("{e:?}") })?
                    {
                        ContinueLifting::Continue(next_addr) => addr = next_addr,
                        ContinueLifting::Stop => break,
                    }

                    instruction_count += 1;
                    if let Some(max_instructions) = config.max_instructions
                        && instruction_count >= max_instructions.into()
                    {
                        break;
                    }
                }
            }
        }

        // Make sure the block is finalized. If there is no open block, then this does nothing.
        block_builder.finish_block().unwrap();

        let range = start_addr..addr;
        block_builder
            .finish(Some(range))
            .map_err(|_| -> LifterError { todo!() })
    }
}
