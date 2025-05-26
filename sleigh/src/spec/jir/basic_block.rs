pub mod builder;
mod display;
pub(crate) mod instruction;
pub mod optimizer;

use crate::spec::jir::basic_block::instruction::JIRTempId;
use crate::spec::jir::jcode::{
    JIRInstruction,
    NextBlockTarget,
};
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockId,
};
use crate::spec::jir::values::JIRVarNode;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use optimizer::OptimizerAncillary;
use std::ops::Deref;
use std::rc::Rc;

pub type InstId = usize;

/// A target basic block contains multiple [TargetInstruction]s and also keeps track of
/// the predecessors and successor. Cloning this structure simply creates a new pointer
/// until the structure is accessed mutably, upon which the content is cloned.
#[derive(Debug, Clone)]
pub struct TargetBasicBlock(pub Rc<TargetBasicBlockInner>);

impl TargetBasicBlock {
    pub fn clone_with_instructions(
        &self,
        jir_instructions: Vec<JIRInstruction>,
    ) -> TargetBasicBlock {
        let mut slf = self.0.deref().clone();
        slf.jir_instructions = jir_instructions;
        Self(Rc::new(slf))
    }

    /// Get an iterator over all instructions of the target basic block.
    /// The instructions are ordered such that the instructions of the next fallthrough block
    /// follow the ones of the current block.
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = &JIRInstruction> + DoubleEndedIterator {
        self.0.jir_instructions.iter()
    }

    /// See [Self::instructions]
    pub fn instructions_with_ids(
        &self
    ) -> impl ExactSizeIterator<Item = (InstId, &JIRInstruction)> + DoubleEndedIterator {
        self.0.jir_instructions.iter().enumerate()
    }

    pub fn instruction_count(&self) -> usize {
        self.0.jir_instructions.len()
    }

    pub fn target_instruction_count(&self) -> usize {
        self.0.target_inst_count
    }

    /// Get an iterator over all instructions in the target basic block. This includes all instructions from
    /// all jir blocks.
    pub fn inst_ids(&self) -> impl ExactSizeIterator<Item = InstId> + DoubleEndedIterator {
        (0..self.instruction_count()).map(|i| i as InstId)
    }

    pub fn jir_block_ids(&self) -> impl ExactSizeIterator<Item = JIRBlockId> + DoubleEndedIterator {
        self.0.jir_blocks.keys()
    }

    /// Get an iterator over all instructions of the provided jir block in the target block.
    pub fn jir_block_inst_ids(
        &self,
        block: JIRBlockId,
    ) -> impl ExactSizeIterator<Item = InstId> + DoubleEndedIterator {
        self.0
            .jir_blocks
            .get(block)
            .map(|block| block.as_ref().map(|b| b.range.clone()))
            .flatten()
            // Return an empty iterator if there is no corresponding block.
            .unwrap_or(0..0)
    }

    pub fn jir_block_insts(
        &self,
        block: JIRBlockId,
    ) -> impl ExactSizeIterator<Item = (InstId, &JIRInstruction)> + DoubleEndedIterator {
        let range = self
            .0
            .jir_blocks
            .get(block)
            .map(|block| block.as_ref().map(|block| block.range.clone()))
            .flatten();
        self.0.jir_instructions[range.unwrap_or(0..0)]
            .iter()
            .enumerate()
    }

    /// The address where this block starts.
    pub fn start(&self) -> EVAddr {
        self.0.start
    }

    /// The address of the next block, if the next block can be reached without branching.
    pub fn fallthrough(&self) -> Option<EVAddr> {
        self.0.fallthrough
    }

    /// Determines whether the block branches, or simply falls through to the next instruction.
    pub fn branches(&self) -> bool {
        self.0.branch_targets.is_some()
    }

    /// If the block branches conditionally or unconditionally, this returns a list of targets for the branches
    /// at the end of this block. In most cases, this will be either one or two targets.
    /// Some obscure architectures may provide more than two branch targets.
    pub fn branch_targets(&self) -> Option<&[NextBlockTarget]> {
        self.0.branch_targets.as_deref()
    }
}

enum BlockTermination {
    Fallthrough(usize),
    Terminate(usize),
}

#[derive(Debug, Clone)]
pub struct TargetBasicBlockInner {
    jir_instructions: Vec<JIRInstruction>,
    /// Keeps track of the blocks and what range of instructions the block lives in.
    jir_blocks: PrimaryMap<JIRBlockId, Option<JIRBlock>>,
    jir_block_predecessors: SecondaryMap<JIRBlockId, Vec<JIRBlockId>>,
    jir_block_successors: SecondaryMap<JIRBlockId, Vec<JIRBlockId>>,

    /// A list of temporary varnodes that are valid only for this TargetBasicBlock.
    /// The temporaries are allocated such that they are sequential in the
    /// address space. This means allocating a new temporary should occur
    /// after the offset+length of the last temporary.
    temporaries: PrimaryMap<JIRTempId, JIRVarNode>,
    /// A list of predecessor blocks that refer to this basic block.
    // predecessors: Vec<EVAddr>,
    start: EVAddr,
    /// The successor of this basic block. Some blocks may not have any successor.
    /// This might be the case if the block issues some kind of halt instruction
    /// that stops execution.
    fallthrough: Option<EVAddr>,
    branch_targets: Option<Vec<NextBlockTarget>>,
    /// The number of target instructions contained in this block.
    target_inst_count: usize,

    /// Ancillary information calculated by the optimizer can be stored here
    optimizer_ancillary: OptimizerAncillary,
}
