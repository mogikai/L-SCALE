use crate::spec::jir::basic_block::TargetBasicBlockBuilder;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::scope::LabelId;
use crate::spec::{
    ProcessorSpecResult,
    SpaceId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use mogi_utilities::new_map_key_16;

new_map_key_16! { pub struct JIRTempId; "JTMP"; }

pub struct TargetInstructionBuilder<'a> {
    /// The instruction builder builds into the basic block builder that is attached.
    block_builder: &'a mut TargetBasicBlockBuilder,
    /// Labels are local to instructions. We keep track of the mapping in the instruction builder.
    /// Once the instruction is built, there are only references to block ids.
    block_labels: SecondaryMap<LabelId, Option<JIRBlockId>>,
    start: Option<EVAddr>,
    next: EVAddr,
}

impl<'a> TargetInstructionBuilder<'a> {
    pub fn new(
        block_builder: &'a mut TargetBasicBlockBuilder,
        start: EVAddr,
        next: EVAddr,
    ) -> Self {
        Self {
            block_builder,
            block_labels: Default::default(),
            start: Some(start),
            next,
        }
    }

    pub fn start_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        if let Some(start) = self.start.take() {
            // If this is the first block, we add an instruction start marker.
            self.block_builder.start_instruction(start)?;
        }
        self.block_builder.start_block()
    }

    pub fn finish_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        self.block_builder.finish_block()
    }

    pub fn push_jir_instruction(
        &mut self,
        instruction: JIRInstruction,
    ) -> ProcessorSpecResult {
        self.block_builder.push_jir_instruction(instruction)
    }

    pub fn alloc_temporary(
        &mut self,
        size: JIRSize,
    ) -> ProcessorSpecResult<JIRSlice> {
        self.block_builder.alloc_temporary(size)
    }

    pub fn load_temp(
        &mut self,
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    ) -> ProcessorSpecResult<JIRSlice> {
        self.block_builder.load_temp(address, space, size)
    }

    pub fn start_labeled_block(
        &mut self,
        label: LabelId,
    ) -> ProcessorSpecResult<JIRBlockId> {
        let block_id = self.resolve_label_to_block(label);
        self.block_builder.manifest_reserved_block(block_id)?;
        Ok(block_id)
    }

    pub fn resolve_label_to_block(
        &mut self,
        label: LabelId,
    ) -> JIRBlockId {
        // if no block exists yet, we insert a None entry that will later be replaced by the labeled block.
        *self.block_labels[label].get_or_insert_with(|| self.block_builder.reserve_block())
    }

    pub fn finish(mut self) -> ProcessorSpecResult {
        self.block_builder.end_instruction(self.next)?;
        Ok(())
    }
}
