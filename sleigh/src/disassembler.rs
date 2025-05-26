use std::fmt::Display;
use std::ops::Range;

use mogi_utilities::addresses::EVAddr;
use thiserror::Error;

use crate::lifter::context_provider::ContextProvider;
use crate::lifter::source::TargetInstructionSource;
use crate::spec::ProcessorSpecificationError;

pub type DisassemblerResult<T = ()> = Result<T, DisassemblerError>;

#[derive(Debug, Error)]
pub enum DisassemblerError {
    #[error("An unknown error occurred")]
    Unknown,
    #[error("The processor spec encountered an error")]
    ProcessorSpecError(#[from] ProcessorSpecificationError),
}

#[derive(Debug)]
pub struct DisassembledInstruction {
    addr: EVAddr,
    size: usize,
    display: String,
}

impl DisassembledInstruction {
    pub fn new(
        addr: EVAddr,
        size: usize,
        display: String,
    ) -> Self {
        Self { addr, size, display }
    }
}

impl Display for DisassembledInstruction {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "0x{:08X}  {:02}   {}", self.addr, self.size, self.display)
    }
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
    ) -> DisassemblerResult<DisassembledInstruction>;

    fn disassemble_block<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr: EVAddr,
    ) -> DisassemblerResult<Vec<DisassembledInstruction>>;

    fn disassemble_range<S: TargetInstructionSource, C: ContextProvider>(
        &self,
        source: &S,
        context_provider: &mut C,
        addr_range: Range<EVAddr>,
    ) -> DisassemblerResult<Vec<DisassembledInstruction>>;
}
