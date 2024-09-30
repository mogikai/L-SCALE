use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::PrimaryMap;
use mogi_utilities::new_map_key;
use std::rc::Rc;

/// A lifted target instruction. It contains information like its address, and the address of the next instruction,
/// and also the JIR instructions that represent its semantic meaning.
///
/// The JIR instructions may use temporary [JIRVarNode](crate::spec::jir::values::JIRVarNode)s.
/// These VarNodes are not visible outside the blocks contained in this target instruction.
/// A JIT implementation thus does not have to flush any of them to the CPU state after the
/// instruction has been emulated.
#[derive(Debug, Clone)]
pub struct TargetInstruction(Rc<TargetInstructionInner>);

#[derive(Debug, Clone)]
struct TargetInstructionInner {
    /// The jir instructions for each JIRBlock are stored in a single vec. Instructions for a block
    /// must be in a sequence in the vec.
    jir_instructions: Vec<JIRInstruction>,

    /// The JIR blocks of the lifted instruction. PCode allows internal jumps, hence we
    /// have one or more blocks per target instruction. PCode relative jumps always jump
    /// inside the blocks of the instruction. If a jump target lies outside the instruction,
    /// then this will be an absolute jump to an [EVAddr].
    jir_blocks: PrimaryMap<JIRBlockId, JIRBlock>,

    /// The [EVAddr] where the instruction starts
    start: EVAddr,
    /// The [EVAddr] that points to the bytes after this instruction, i.e. the next instruction.
    next: EVAddr,
}

new_map_key! { pub struct TargetInstructionId; "TINST"; }

impl TargetInstruction {
    /// Creates a new instruction that initially only contains one block.
    pub fn new(
        start: EVAddr,
        next: EVAddr,
    ) -> Self {
        Self(Rc::new(TargetInstructionInner {
            jir_instructions: vec![],
            jir_blocks: Default::default(),
            start,
            next,
        }))
    }

    pub fn next(&self) -> EVAddr {
        self.0.next
    }
}
