use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockId,
};
use slotmap::SlotMap;

/// The lifter cache contains all the translated instructions and blocks.
pub struct LifterCache {
    /// The jir instructions for each JIRBlock are stored in a single vec. Instructions for a block
    /// must be in a sequence in the vec.
    jir_instructions: Vec<JIRInstruction>,
    jir_blocks: SlotMap<JIRBlockId, JIRBlock>,

    target_instructions: Vec<TargetInstruction>,

    current_instruction: TargetInstruction,
    current_jir_block: JIRBlock,
}

impl LifterCache {
    /// Push a JIR instruction to the instruction stream.
    pub fn push_instruction(
        &mut self,
        instruction: JIRInstruction,
    ) {
    }
}
