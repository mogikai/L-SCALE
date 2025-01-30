mod display;

use crate::jinst;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::instruction::JIRTempId;
use crate::spec::jir::jcode::JIRInstruction;
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
use mogi_utilities::index_map::PrimaryMap;
use std::rc::Rc;

pub struct TargetBasicBlockBuilder {
    target_block: TargetBasicBlockInner,
    current_block_id: Option<JIRBlockId>,
    previous_block_id: Option<JIRBlockId>,
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
                temporaries: Default::default(),
                start,
                fallthrough: None,
            },
            current_block_id: None,
            previous_block_id: None,
            temp_space,
        }
    }

    pub fn alloc_temporary(
        &mut self,
        size: JIRSize,
    ) -> ProcessorSpecResult<JIRSlice> {
        // TODO: Maybe it makes sense padding the varnodes to a specific byte boundary?
        // The offset of the new temporary is either the end of the last, or zero, if no other temps were allocated yet.
        let offset: JIRSize = self
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

    pub fn start_instruction(
        &mut self,
        address: EVAddr,
    ) -> ProcessorSpecResult {
        self.target_block
            .jir_instructions
            .push(JIRInstruction::instruction_start(address));
        Ok(())
    }

    pub fn end_instruction(
        &mut self,
        address: EVAddr,
    ) -> ProcessorSpecResult {
        self.target_block
            .jir_instructions
            .push(JIRInstruction::instruction_end(address));
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

    pub fn finish(self) -> ProcessorSpecResult<TargetBasicBlock> {
        Ok(TargetBasicBlock(Rc::new(self.target_block)))
    }
}

/// A target basic block contains multiple [TargetInstruction]s and also keeps track of
/// the predecessors and successor. Cloning this structure simply creates a new pointer
/// until the structure is accessed mutably, upon which the content is cloned.
#[derive(Debug, Clone)]
pub struct TargetBasicBlock(Rc<TargetBasicBlockInner>);

impl TargetBasicBlock {}

#[derive(Debug, Clone)]
struct TargetBasicBlockInner {
    jir_instructions: Vec<JIRInstruction>,
    jir_blocks: PrimaryMap<JIRBlockId, Option<JIRBlock>>,

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
}
