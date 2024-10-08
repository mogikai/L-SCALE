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

    block_labels: SecondaryMap<LabelId, Option<JIRBlockId>>,

    /// The [EVAddr] where the instruction starts
    start: EVAddr,
    /// The [EVAddr] that points to the bytes after this instruction, i.e. the next instruction.
    next: EVAddr,
}

new_map_key! { pub struct TargetInstructionId; "TINST"; }

pub struct JIRBlockBuilder<'a> {
    /// The offset in `jir_instructions` where this block starts.
    start_offset: usize,
    instruction: &'a mut TargetInstruction,
    /// Tracks if the block was finalized. If not, then all written instructions are discarded.
    finalized: bool,
    block_id: JIRBlockId,
}

impl JIRBlockBuilder<'_> {
    pub fn resolve_label_to_block(
        &mut self,
        label: LabelId,
    ) -> JIRBlockId {
        self.instruction.resolve_label_to_block(label)
    }

    /// Adds a jir instruction to this block.
    /// This operation may perform a clone if the target instruction was lazily cloned beforehand.
    pub fn push_jir_instruction(
        &mut self,
        jir_instruction: JIRInstruction,
    ) {
        Rc::make_mut(&mut self.instruction.0)
            .jir_instructions
            .push(jir_instruction);
    }

    /// Commits this block to the target instruction, returning its block id.
    /// The provided previous_block will be modified to point to block being finalized.
    pub fn finalize(
        mut self,
        previous_block: Option<JIRBlockId>,
    ) -> JIRBlockId {
        self.finalized = true;
        let instruction = Rc::make_mut(&mut self.instruction.0);
        instruction.jir_blocks[self.block_id] = Some(JIRBlock {
            range: self.start_offset..instruction.jir_instructions.len(),
            fallthrough: JIRBlockFallthrough::NextInst,
        });
        if let Some(previous_block) = previous_block.and_then(|block| {
            instruction
                .jir_blocks
                .get_mut(block)
                .map(Option::as_mut)
                .flatten()
        }) {
            previous_block.fallthrough = JIRBlockFallthrough::Block(self.block_id);
        }
        self.block_id
    }
}

impl Drop for JIRBlockBuilder<'_> {
    fn drop(&mut self) {
        if !self.finalized {
            Rc::make_mut(&mut self.instruction.0)
                .jir_instructions
                .truncate(self.start_offset);
        }
    }
}

impl TargetInstruction {
    /// Creates a new instruction that initially only contains one block.
    pub fn new(
        start: EVAddr,
        next: EVAddr,
    ) -> Self {
        Self(Rc::new(TargetInstructionInner {
            jir_instructions: vec![],
            jir_blocks: Default::default(),
            block_labels: Default::default(),
            start,
            next,
        }))
    }

    /// Get a block builder that enables adding instructions the current block.
    /// Once the builder is dropped, the block is finalized.
    pub fn next_block_builder(&mut self) -> JIRBlockBuilder {
        let inst = Rc::make_mut(&mut self.0);
        // We insert a None block to reserve this id. This is mainly relevant for when
        // we encounter jumps to blocks that have not been defined yet. These will receive
        // an id allocation, but will be constructed later on.
        let block_id = inst.jir_blocks.insert(None);
        JIRBlockBuilder {
            start_offset: self.0.jir_instructions.len(),
            instruction: self,
            finalized: false,
            block_id,
        }
    }

    /// Get a block builder for a labeled block. This either creates a new entry in the label mapping
    /// or pulls the already assigned block id from the label mapping, such that jumps refer to the
    /// correct block id
    pub fn block_builder_for_label(
        &mut self,
        label: LabelId,
    ) -> JIRBlockBuilder {
        let inst = Rc::make_mut(&mut self.0);
        let block_id = &mut inst.block_labels[label];
        let block_id = *block_id.get_or_insert_with(|| inst.jir_blocks.insert(None));
        JIRBlockBuilder {
            start_offset: 0,
            instruction: self,
            finalized: false,
            block_id,
        }
    }

    pub fn resolve_label_to_block(
        &mut self,
        label: LabelId,
    ) -> JIRBlockId {
        let inst = Rc::make_mut(&mut self.0);
        // if no block exists yet, we insert a None entry that will later be replaced by the labeled block.
        *inst.block_labels[label].get_or_insert_with(|| inst.jir_blocks.insert(None))
    }

    pub fn next(&self) -> EVAddr {
        self.0.next
    }

    fn mut_inner(&mut self) -> &mut TargetInstructionInner {
        Rc::make_mut(&mut self.0)
    }
}
