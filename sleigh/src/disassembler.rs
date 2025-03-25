use mogi_utilities::addresses::EVAddr;
use thiserror::Error;

use crate::{lifter::{context_provider::ContextProvider, source::TargetInstructionSource}, spec::ProcessorSpecificationError};

pub type DisassemblerResult<T = ()> = Result<T, DisassemblerError>;

#[derive(Debug, Error)]
pub enum DisassemblerError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The processor spec encountered an error")]
    ProcessorSpecError(#[from] ProcessorSpecificationError)
}

/// The disassembler trait defines the functionality of a disassembler.
/// Disassembling is the process of decoding one or more target instructions to a human-readable
/// display string.
pub trait Disassembler {
    /// Disassemble a single instruction by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction and the display string
    /// of this single instruction.
    /// To disassemble the entire target basic block, use [Self::disassemble_block] instead.
    fn disassemble_instruction<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
    ) -> DisassemblerResult<String>;

    fn disassemble_block<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
    ) -> DisassemblerResult<Vec<String>>;
}
