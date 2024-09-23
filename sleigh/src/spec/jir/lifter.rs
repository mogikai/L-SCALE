use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::source::TargetInstructionSource;
use mogi_utilities::addresses::EVAddr;
use thiserror::Error;

pub type JIRLifterResult<T = ()> = Result<T, JIRLifterError>;

pub trait JIRLifter {
    /// Lift a single instruction if possible, by fetching the bytes for the instruction from
    /// `source`. Returns the address of the next instruction, and the lifted instruction
    /// One target instruction corresponds to multiple lifted instructions most of the time.
    /// See [TargetInstruction] for more detail.
    fn lift(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
    ) -> JIRLifterResult<(EVAddr, TargetInstruction)>;

    /// Does the same as [lift](Self::lift), but tries to lift an entire TargetBasicBlock
    fn lift_block(
        &self,
        addr: EVAddr,
        source: impl TargetInstructionSource,
    ) -> JIRLifterResult<(EVAddr, Vec<TargetInstruction>)>;

    fn disasm(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, String)>;

    fn disasm_many(
        &self,
        data: &[u8],
    ) -> JIRLifterResult<(usize, Vec<String>)>;
}

#[derive(Debug, Error)]
pub enum JIRLifterError {
    #[error("An unknown error occurred")]
    Unknown,
}
