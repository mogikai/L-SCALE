use crate::spec::jir::jir_block::JIRBlock;
use mogi_utilities::addresses::EVAddr;

/// A lifted target instruction. It contains information like its address, and the address of the next instruction,
/// and also the JIR instructions that represent its semantic meaning.
///
/// The JIR instructions may use temporary [JIRVarNode](crate::spec::jir::values::JIRVarNode)s.
/// These VarNodes are not visible outside the blocks contained in this target instruction.
/// A JIT implementation thus does not have to flush any of them to the CPU state after the
/// instruction has been emulated.
#[derive(Debug, Default)]
pub struct TargetInstruction {
    /// The JIR blocks of the lifted instruction. PCode allows internal jumps, hence we
    /// have one or more blocks per target instruction. PCode relative jumps always jump
    /// inside the blocks of the instruction. If a jump target lies outside the instruction,
    /// then this will be an absolute jump to an [EVAddr].
    blocks: Vec<JIRBlock>,
    start: EVAddr,
    next: EVAddr,
}
