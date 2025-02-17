use crate::lifter::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::lifter::{
    Lifter,
    LifterDiagnostics,
    LifterError,
    LifterResult,
};
use crate::spec::ProcessorSpecification;
use crate::spec::jir::basic_block::{
    TargetBasicBlock,
    TargetBasicBlockBuilder,
};
use crate::spec::jir::context_stack::ContextStack;
use crate::spec::jir::jumplist::MatcherResult;
use crate::spec::jir::trace_jir;
use mogi_utilities::addresses::EVAddr;
use num::PrimInt;
use std::cell::Ref;
use std::time::Instant;

impl Lifter for ProcessorSpecification {
    fn lift_instruction<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
    ) -> LifterResult<(EVAddr, TargetBasicBlock)> {
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
        self.lifter_diagnostics
            .borrow_mut()
            .inst_lifted_since_last_reset += 1;

        // TODO: Allocate buffer size depending on the target instruction sets maximum instruction size.
        let mut buffer = [0u8; 16];
        source
            .load(addr, &mut buffer)
            .map_err(|_| -> LifterError { todo!() })?;

        let now = Instant::now();
        let root_jumplist = self.jumplist_cache.get(0.into()).unwrap();

        let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
        let matching_result = root_jumplist
            .r#match(&buffer, addr, &self.jumplist_cache, &self.symbol_tables.constructors, &mut context_stack)
            .unwrap();
        self.lifter_diagnostics.borrow_mut().total_time_match += now.elapsed();

        let now = Instant::now();
        match matching_result {
            MatcherResult::EndOfByteStream => todo!("Handle end of byte stream"),
            MatcherResult::None => todo!("Handle no match"),
            MatcherResult::Some {
                table: _,
                resolver,
                bytes_consumed: _,
            } => {
                let mut block_builder = TargetBasicBlockBuilder::new(addr, self.temp_space);
                resolver.print_hierarchy(0);

                resolver
                    .build(self, &mut block_builder)
                    .map_err(|_| -> LifterError { todo!() })?;

                let block = block_builder
                    .finish()
                    .map_err(|_| -> LifterError { todo!() })?;
                self.lifter_diagnostics.borrow_mut().total_time_build += now.elapsed();
                Ok((addr, block))
            }
        }
    }

    fn lift_block<S, C>(
        &self,
        source: &S,
        context_provider: &mut C,
        mut addr: EVAddr,
    ) -> LifterResult<(EVAddr, TargetBasicBlock)>
    where
        S: TargetInstructionSource,
        C: ContextProvider,
    {
        // TODO: Allocate buffer size depending on the target instruction sets maximum instruction size.
        let mut buffer = [0u8; 16];

        let now = Instant::now();
        let root_jumplist = self.jumplist_cache.get(0.into()).unwrap();
        let mut block_builder = TargetBasicBlockBuilder::new(addr, self.temp_space);

        loop {
            trace_jir!("Loading instruction bytes at {:?}", addr);
            source
                .load(addr, &mut buffer)
                .map_err(|_| -> LifterError { todo!() })?;

            // TODO error type
            let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
            // TODO: Initially fill the context stack with the value from the context provider that needs to be passed in.
            // longMode=1, addrSize=1, opSize=1

            let matching_result = root_jumplist
                .r#match(&buffer, addr, &self.jumplist_cache, &self.symbol_tables.constructors, &mut context_stack)
                .unwrap();
            self.lifter_diagnostics.borrow_mut().total_time_match += now.elapsed();

            let now = Instant::now();
            match matching_result {
                MatcherResult::EndOfByteStream => todo!("Handle end of byte stream"),
                MatcherResult::None => todo!("Handle no match"),
                MatcherResult::Some {
                    table: _,
                    resolver,
                    bytes_consumed,
                } => {
                    trace_jir!("Found match and consumed {} bytes", bytes_consumed);

                    resolver.print_hierarchy(0);
                    addr += bytes_consumed;

                    let block_end = resolver.constructor().block_end;
                    resolver
                        .build(self, &mut block_builder)
                        .map_err(|_| -> LifterError { todo!() })?;
                    self.lifter_diagnostics.borrow_mut().total_time_build += now.elapsed();

                    if block_end {
                        break;
                    }
                }
            }
        }

        Ok((
            addr,
            block_builder
                .finish()
                .map_err(|_| -> LifterError { todo!() })?,
        ))
    }

    fn diagnostics(&self) -> Ref<LifterDiagnostics> {
        self.lifter_diagnostics.borrow()
    }

    fn reset_diagnostics(&mut self) {
        *self.lifter_diagnostics.borrow_mut() = LifterDiagnostics::default();
    }
}
