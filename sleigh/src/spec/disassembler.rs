use std::ops::Range;

use mogi_utilities::addresses::EVAddr;

use crate::disassembler::{
    DisassembledInstruction,
    Disassembler,
    DisassemblerError,
    DisassemblerResult,
};
use crate::lifter::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::spec::jir::jcode::ContinueLifting;
use crate::spec::jir::trace_jir;

use super::ProcessorSpecification;
use super::jir::basic_block::builder::TargetBasicBlockBuilder;
use super::jir::context_stack::ContextStack;
use super::jir::jumplist::{
    MatchContext,
    MatcherResult,
};

impl Disassembler for ProcessorSpecification {
    fn disassemble_instruction<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
    ) -> DisassemblerResult<DisassembledInstruction> {
        // TODO: Allocate buffer size depending on the target instruction sets maximum instruction size.
        let mut buffer = [0u8; 16];
        source
            .load(addr, &mut buffer)
            .map_err(|_| -> DisassemblerError { todo!() })?;

        let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
        let match_context = MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors);
        let matching_result = match_context.r#match(&buffer, &mut context_stack).unwrap();

        match matching_result {
            MatcherResult::EndOfByteStream => todo!("Handle end of byte stream"),
            MatcherResult::None => todo!("Handle no match"),
            MatcherResult::Some {
                table: _,
                resolver,
                bytes_consumed,
            } => {
                let display = resolver
                    .build_display(self)
                    .map_err(|_| -> DisassemblerError { todo!() })?;
                Ok(DisassembledInstruction::new(addr, bytes_consumed, display))
            }
        }
    }

    fn disassemble_block<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        mut addr: EVAddr,
    ) -> DisassemblerResult<Vec<DisassembledInstruction>> {
        // TODO: Allocate buffer size depending on the target instruction set's maximum instruction size.
        let mut buffer = [0u8; 16];
        let mut disassembly = Vec::new();

        // also lift block until end (TODO better way to determine block end)
        let mut block_builder = TargetBasicBlockBuilder::new(addr, self.temp_space);
        block_builder.start_block().unwrap();

        loop {
            trace_jir!("Loading instruction bytes at {:?}", addr);
            source
                .load(addr, &mut buffer)
                .map_err(|_| -> DisassemblerError { todo!() })?;

            let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
            let match_context = MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors);
            let matching_result = match_context.r#match(&buffer, &mut context_stack).unwrap();

            #[allow(unused_variables)]
            match matching_result {
                MatcherResult::EndOfByteStream => todo!("Handle end of byte stream"),
                MatcherResult::None => todo!("Handle no match"),
                MatcherResult::Some {
                    table: _,
                    resolver,
                    bytes_consumed,
                } => {
                    trace_jir!("Found match and consumed {} bytes", bytes_consumed);
                    trace_jir!("{}", resolver.hierarchy_debug_string().unwrap());

                    trace_jir!("{}", resolver.mcode_debug_string(&self).unwrap());

                    let display = resolver.build_display(self)?;
                    let instr = DisassembledInstruction::new(addr, bytes_consumed, display);
                    disassembly.push(instr);

                    match resolver
                        .build(self, &mut block_builder)
                        .map_err(|e| -> DisassemblerError { todo!("{e:?}") })?
                    {
                        ContinueLifting::Continue(next_addr) => addr = next_addr,
                        ContinueLifting::Stop => break,
                    }
                }
            }
        }

        Ok(disassembly)
    }

    fn disassemble_range<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr_range: Range<EVAddr>,
    ) -> DisassemblerResult<Vec<DisassembledInstruction>> {
        let mut addr = addr_range.start;
        let mut disassembly = Vec::new();

        while addr < addr_range.end {
            trace_jir!("Disassembling instruction at {:?}", addr);

            // TODO: Allocate buffer size depending on the target instruction sets maximum instruction size.
            let mut buffer = [0u8; 16];
            source
                .load(addr, &mut buffer)
                .map_err(|_| -> DisassemblerError { todo!() })?;

            let mut context_stack = ContextStack::new(self, context_provider).expect("Couldn't create context stack");
            let match_context = MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors);
            let matching_result = match_context.r#match(&buffer, &mut context_stack).unwrap();

            match matching_result {
                MatcherResult::EndOfByteStream => todo!("Handle end of byte stream"),
                MatcherResult::None => todo!("Handle no match"),
                MatcherResult::Some {
                    table: _,
                    resolver,
                    bytes_consumed,
                } => {
                    let display = resolver
                        .build_display(self)
                        .map_err(|_| -> DisassemblerError { todo!() })?;
                    let instr = DisassembledInstruction::new(addr, bytes_consumed, display);
                    disassembly.push(instr);
                    addr += bytes_consumed;
                }
            }
        }

        Ok(disassembly)
    }
}
