pub mod display;

use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockFallthrough,
    JIRBlockId,
};
use crate::spec::moir::scope::LabelId;
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
};
use itertools::Itertools;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
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

pub struct TargetInstructionBuilder {
    instruction: TargetInstructionInner,
    current_block_id: Option<JIRBlockId>,
    previous_block_id: Option<JIRBlockId>,
    block_labels: SecondaryMap<LabelId, Option<JIRBlockId>>,
}

#[derive(Debug, Clone)]
struct TargetInstructionInner {
    /// The jir instructions for each JIRBlock are stored in a single vec. Instructions for a block
    /// must be in a sequence in the vec.
    jir_instructions: Vec<JIRInstruction>,

    /// The JIR blocks of the lifted instruction. PCode allows internal jumps, hence we
    /// have one or more blocks per target instruction. PCode relative jumps always jump
    /// inside the blocks of the instruction. If a jump target lies outside the instruction,
    /// then this will be an absolute jump to an [EVAddr].
    /// Instructions always start execution with the first block.
    jir_blocks: PrimaryMap<JIRBlockId, Option<JIRBlock>>,

    /// The [EVAddr] where the instruction starts
    start: EVAddr,
    /// The [EVAddr] that points to the bytes after this instruction, i.e. the next instruction.
    next: EVAddr,
}

new_map_key! { pub struct TargetInstructionId; "TINST"; }

impl TargetInstructionBuilder {
    pub fn new(
        start: EVAddr,
        next: EVAddr,
    ) -> Self {
        Self {
            instruction: TargetInstructionInner {
                jir_instructions: vec![],
                jir_blocks: Default::default(),
                start,
                next,
            },
            current_block_id: None,
            previous_block_id: None,
            block_labels: Default::default(),
        }
    }

    pub fn push_jir_instruction(
        &mut self,
        instruction: JIRInstruction,
    ) -> ProcessorSpecResult {
        if self.current_block_id.is_none() {
            todo!("Pushing instructions requires an active block")
        }
        self.instruction.jir_instructions.push(instruction);
        Ok(())
    }

    pub fn start_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        if self.current_block_id.is_some() {
            todo!("Error for 'Cannot start a new block without finalizing the existing block'")
        }
        let block_id = self.instruction.jir_blocks.insert(Some(JIRBlock {
            range: self.instruction.jir_instructions.len()..self.instruction.jir_instructions.len(),
            fallthrough: JIRBlockFallthrough::NextInst,
        }));
        // Link the previous blocks fallthrough to the new block
        if let Some(previous_block) = self.previous_block_id {
            self.instruction.jir_blocks[previous_block]
                .as_mut()
                .expect("If a previous block exists, it must have been finalized")
                .fallthrough = JIRBlockFallthrough::Block(block_id);
        }
        self.current_block_id = Some(block_id);
        Ok(block_id)
    }

    pub fn start_labeled_block(
        &mut self,
        label: LabelId,
    ) -> ProcessorSpecResult<JIRBlockId> {
        if self.current_block_id.is_some() {
            todo!("Error for 'Cannot start a new block without finalizing the existing block'")
        }
        // Get the pre-allocated block or insert a new one.
        let block_id = *self.block_labels[label].get_or_insert_with(|| self.instruction.jir_blocks.insert(None));
        self.instruction.jir_blocks[block_id] = Some(JIRBlock {
            range: self.instruction.jir_instructions.len()..self.instruction.jir_instructions.len(),
            fallthrough: JIRBlockFallthrough::NextInst,
        });
        // Link the previous blocks fallthrough to the new block
        if let Some(previous_block) = self.previous_block_id {
            self.instruction.jir_blocks[previous_block]
                .as_mut()
                .expect("If a previous block exists, it must have been finalized")
                .fallthrough = JIRBlockFallthrough::Block(block_id);
        }
        self.current_block_id = Some(block_id);
        Ok(block_id)
    }

    pub fn finish_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        let Some(block_id) = self.current_block_id else {
            todo!("Error for 'Finished block without having started a block'")
        };
        // Update the end of the range to the current instructions vec end.
        self.instruction.jir_blocks[block_id]
            .as_mut()
            .expect("If there is an active current block, it must be initialized")
            .range
            .end = self.instruction.jir_instructions.len();
        self.previous_block_id = Some(block_id);
        self.current_block_id = None;
        Ok(block_id)
    }

    pub fn resolve_label_to_block(
        &mut self,
        label: LabelId,
    ) -> JIRBlockId {
        // if no block exists yet, we insert a None entry that will later be replaced by the labeled block.
        *self.block_labels[label].get_or_insert_with(|| self.instruction.jir_blocks.insert(None))
    }

    pub fn finish(self) -> TargetInstruction {
        TargetInstruction(Rc::new(self.instruction))
    }
}

impl TargetInstruction {
    pub fn next(&self) -> EVAddr {
        self.0.next
    }

    fn mut_inner(&mut self) -> &mut TargetInstructionInner {
        Rc::make_mut(&mut self.0)
    }
}
