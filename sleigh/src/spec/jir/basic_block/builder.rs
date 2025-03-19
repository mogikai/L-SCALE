use crate::jinst;
use crate::spec::jir::basic_block::instruction::TargetInstructionBuilder;
use crate::spec::jir::basic_block::{
    TargetBasicBlock,
    TargetBasicBlockInner,
};
use crate::spec::jir::common::{
    JIROffset,
    JIRSize,
};
use crate::spec::jir::jcode::{
    JIRInstruction,
    NextBlockTarget,
};
use crate::spec::jir::jir_block::{
    JIRBlock,
    JIRBlockFallthrough,
    JIRBlockId,
};
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
    JIRVarNode,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
    SpaceId,
};
use mogi_utilities::addresses::EVAddr;
use std::rc::Rc;

/// A builder for constructing `TargetBasicBlock` structures.
///
/// This builder manages the internal state of the basic block,
/// including its instructions, temporaries, and connections
/// between blocks. It provides methods for starting and finishing
/// blocks, allocating temporaries, and managing control flow
/// branches.
pub struct TargetBasicBlockBuilder {
    /// The inner representation of the target basic block being constructed.
    target_block: TargetBasicBlockInner,

    /// The ID of the currently active block. If `None`, no block is currently active.
    current_block_id: Option<JIRBlockId>,

    /// The ID of the previously finalized block. This is used to link consecutive blocks.
    previous_block_id: Option<JIRBlockId>,

    /// Keeps track of the next instruction to be added. If the block is finalized before
    /// a branch occurs, this instruction is set as the fallthrough.
    next_instruction: Option<EVAddr>,

    /// The address space used for allocating temporary variables within the basic block.
    temp_space: SpaceId,
}

impl TargetBasicBlockBuilder {
    pub fn new(
        start: EVAddr,
        temp_space: SpaceId,
    ) -> Self {
        Self {
            target_block: TargetBasicBlockInner {
                jir_instructions: vec![],
                jir_blocks: Default::default(),
                jir_block_predecessors: Default::default(),
                jir_block_successors: Default::default(),
                temporaries: Default::default(),
                start,
                fallthrough: None,
                branch_targets: None,
            },
            current_block_id: None,
            previous_block_id: None,
            next_instruction: None,
            temp_space,
        }
    }

    /// Returns true if the block currently has no successors. This can be helpful
    /// when constructing single instruction blocks, where a manual call to
    /// [set_fallthrough](TargetBasicBlockBuilder::set_fallthrough) will be necessary.
    pub fn no_successor(&self) -> bool {
        self.target_block.fallthrough.is_none() && self.target_block.branch_targets.is_none()
    }

    pub fn set_fallthrough(
        &mut self,
        fallthrough: EVAddr,
    ) {
        self.target_block.fallthrough = Some(fallthrough);
    }

    pub fn alloc_temporary(
        &mut self,
        size: JIRSize,
    ) -> ProcessorSpecResult<JIRSlice> {
        // TODO: Maybe it makes sense padding the varnodes to a specific byte boundary?
        // The offset of the new temporary is either the end of the last, or zero, if no other temps were allocated yet.
        let offset: JIROffset = self
            .target_block
            .temporaries
            .last()
            .map(|v| v.end())
            .unwrap_or(0)
            .try_into()
            .map_err(|_| ProcessorSpecificationError::OutOfTempSpace)?;
        let id = self.target_block.temporaries.insert(JIRVarNode {
            space_id: self.temp_space,
            offset: offset.into(),
            size,
        });
        Ok(JIRSlice::new_temp(id, self.temp_space, offset, size))
    }

    /// Allocate a temporary of size `size` and load the contents at `address` in `space` to it.
    /// Returns the temporary Slice
    pub fn load_temp(
        &mut self,
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    ) -> ProcessorSpecResult<JIRSlice> {
        self.alloc_temporary(size).and_then(|tmp| {
            self.push_jir_instruction(jinst!(tmp = *[space]address))
                .and(Ok(tmp))
        })
    }

    /// Starts building a new instruction by returning a new [TargetInstructionBuilder].
    /// To finalize the instruction, call [TargetInstructionBuilder::finish]
    pub fn start_instruction(
        &mut self,
        start: EVAddr,
        next: EVAddr,
    ) -> ProcessorSpecResult<TargetInstructionBuilder> {
        if let Some(previous_next) = self.next_instruction
            && previous_next != start
        {
            Err(todo!(
                "Report error: Cannot start an instruction that is not directly connected in the control flow."
            ))?
        }
        if self.target_block.fallthrough.is_some() || self.target_block.branch_targets.is_some() {
            // Another instruction has previously emitted a goto
            Err(todo!("Report error: The previous instruction already terminated the block"))?
        }
        self.target_block
            .jir_instructions
            .push(JIRInstruction::instruction_start(start));
        Ok(TargetInstructionBuilder {
            block_builder: self,
            block_labels: Default::default(),
            start,
            fallthrough: next,
            branch_targets: None,
        })
    }

    pub(super) fn end_instruction(
        &mut self,
        address: EVAddr,
        next: EVAddr,
        fallthrough: Option<EVAddr>,
        branch_targets: Option<Vec<NextBlockTarget>>,
    ) -> ProcessorSpecResult {
        self.next_instruction = Some(next);
        self.target_block
            .jir_instructions
            .push(JIRInstruction::instruction_end(address));
        self.target_block.fallthrough = fallthrough;
        self.target_block.branch_targets = branch_targets;
        Ok(())
    }

    pub fn push_jir_instruction(
        &mut self,
        instruction: JIRInstruction,
    ) -> ProcessorSpecResult {
        if self.current_block_id.is_none() {
            todo!("Pushing instructions requires an active block")
        }
        self.target_block.jir_instructions.push(instruction);
        Ok(())
    }

    pub fn is_last_instruction_unconditional_external_branch(&self) -> bool {
        self.target_block
            .jir_instructions
            .last()
            .map(|i| i.is_unconditional_external_branch())
            .unwrap_or(false)
    }

    /// Reserve a new block id for future blocks, such that we can have forward-references.
    pub fn reserve_block(&mut self) -> JIRBlockId {
        self.target_block.jir_blocks.insert(None)
    }

    pub fn start_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        if self.current_block_id.is_some() {
            todo!("Error for 'Cannot start a new block without finalizing the existing block'")
        }
        let block_id = self
            .target_block
            .jir_blocks
            .insert(Some(self.create_block()));
        self.manifest_block(block_id).map(|_| block_id)
    }

    pub fn manifest_reserved_block(
        &mut self,
        block_id: JIRBlockId,
    ) -> ProcessorSpecResult {
        if self.current_block_id.is_some() {
            todo!("Error for 'Cannot start a new block without finalizing the existing block'")
        }
        self.target_block.jir_blocks[block_id] = Some(self.create_block());
        self.manifest_block(block_id)
    }

    fn create_block(&self) -> JIRBlock {
        JIRBlock {
            // +1 because we don't want the block start marker to be a part of the block.
            range: self.target_block.jir_instructions.len() + 1..self.target_block.jir_instructions.len() + 1,
            fallthrough: JIRBlockFallthrough::NextInst,
        }
    }

    fn manifest_block(
        &mut self,
        block_id: JIRBlockId,
    ) -> ProcessorSpecResult {
        // The block start marker is not part of the block, but marks its beginning.
        self.target_block
            .jir_instructions
            .push(JIRInstruction::block_start(block_id));
        // Link the previous blocks fallthrough to the new block
        if let Some(previous_block) = self.previous_block_id {
            self.target_block.jir_blocks[previous_block]
                .as_mut()
                .expect("If a previous block exists, it must have been finalized")
                .fallthrough = JIRBlockFallthrough::Block(block_id);
        }
        self.current_block_id = Some(block_id);
        Ok(())
    }

    pub fn finish_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        let Some(block_id) = self.current_block_id else {
            todo!("Error for 'Finished block without having started a block'")
        };
        // Update the end of the range to the current instructions vec end.
        self.target_block.jir_blocks[block_id]
            .as_mut()
            .expect("If there is an active current block, it must be initialized")
            .range
            .end = self.target_block.jir_instructions.len();
        self.previous_block_id = Some(block_id);
        self.current_block_id = None;
        self.target_block
            .jir_instructions
            .push(JIRInstruction::block_end(block_id));
        Ok(block_id)
    }

    pub fn finish(mut self) -> ProcessorSpecResult<TargetBasicBlock> {
        if self.target_block.branch_targets.is_none() && self.target_block.fallthrough.is_none() {
            // In case only non-branching instructions were added, we want to set the fallthrough
            // to the next instruction in line.
            self.target_block.fallthrough = self.next_instruction
        }
        Ok(TargetBasicBlock(Rc::new(self.target_block)))
    }
}
