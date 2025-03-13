use crate::spec::jir::basic_block::TargetBasicBlockBuilder;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::jcode::{
    ContinueLifting,
    JIRInstruction,
    NextBlockTarget,
};
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::scope::LabelId;
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
    SpaceId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use mogi_utilities::new_map_key_16;

new_map_key_16! { pub struct JIRTempId; "JTMP"; }

pub struct TargetInstructionBuilder<'a> {
    /// The instruction builder builds into the basic block builder that is attached.
    pub(super) block_builder: &'a mut TargetBasicBlockBuilder,
    /// Labels are local to instructions. We keep track of the mapping in the instruction builder.
    /// Once the instruction is built, there are only references to block ids.
    pub(super) block_labels: SecondaryMap<LabelId, Option<JIRBlockId>>,
    pub(super) start: EVAddr,
    /// The address of the instruction that follows this instruction.
    pub(super) fallthrough: EVAddr,
    /// If the instruction branches, this vector contains all the possible branch targets (usually one or two)
    pub(super) branch_targets: Option<Vec<NextBlockTarget>>,
}

impl<'a> TargetInstructionBuilder<'a> {
    pub fn start_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        self.block_builder.start_block()
    }

    pub fn finish_block(&mut self) -> ProcessorSpecResult<JIRBlockId> {
        self.block_builder.finish_block()
    }

    pub fn push_jir_instruction(
        &mut self,
        instruction: JIRInstruction,
    ) -> ProcessorSpecResult {
        /// If we insert a branch instruction, we add the target to the set of out edges.
        /// Once the instruction is finished, the targets are pushed into the block, and
        /// the block is locked for further instructions.
        match instruction {
            JIRInstruction::GotoDirectIf { target, .. }
            | JIRInstruction::CallDirect(target)
            | JIRInstruction::ReturnDirect(target)
            | JIRInstruction::GotoDirect(target) => match target {
                // We also handle a conditional jump here, since the fallthrough only happens
                // at the end of the instruction which is handled when the instruction is finalized.
                JIRValue::Slice(target) => self
                    .branch_targets
                    .get_or_insert_default()
                    .push(NextBlockTarget::DynamicDirect(target.into())),
                JIRValue::Const(target) => self
                    .branch_targets
                    .get_or_insert_default()
                    .push(NextBlockTarget::Direct(target.try_into()?)),
            },
            JIRInstruction::CallIndirect(target)
            | JIRInstruction::ReturnIndirect(target)
            | JIRInstruction::GotoIndirect(target)
            | JIRInstruction::GotoIndirectIf { target, .. } => {
                self.branch_targets
                    .get_or_insert_default()
                    .push(NextBlockTarget::DynamicIndirect(target));
            }
            _ => {}
        }
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

    fn _finish(
        block_builder: &mut TargetBasicBlockBuilder,
        start: EVAddr,
        fallthrough: EVAddr,
        branch_targets: Option<Vec<NextBlockTarget>>,
    ) -> ProcessorSpecResult {
        if block_builder.is_last_instruction_unconditional_external_branch() {
            // The last instruction is an unconditional branch. There is no fallthrough.
            block_builder.end_instruction(start, fallthrough, None, branch_targets)?;
        } else {
            // if we don't have an unconditional branch as last instruction, the target instruction
            // falls through to the next instruction.
            // If we don't have any branch targets, then there is no need to provide a fallthrough,
            // since the block will end later on.
            block_builder.end_instruction(
                start,
                fallthrough,
                branch_targets.as_ref().and_then(|_| Some(fallthrough)),
                branch_targets,
            )?;
        }
        Ok(())
    }

    /// Finish building the instruction. Returns the address of the next instruction, and a boolean
    /// that indicates if this instruction ends the current target block.
    pub fn finish(mut self) -> ProcessorSpecResult<ContinueLifting> {
        let cont = self
            .branch_targets
            .as_ref()
            .map_or(ContinueLifting::Continue(self.fallthrough), |_| ContinueLifting::Stop);
        Self::_finish(self.block_builder, self.start, self.fallthrough, self.branch_targets)?;
        Ok(cont)
    }

    /// The instruction ends with a goto instruction. This necessitates that the instruction is finalized,
    /// and afterward a new block is created.
    pub fn finish_goto(mut self) -> ProcessorSpecResult<ContinueLifting> {
        let cont = self
            .branch_targets
            .as_ref()
            .map_or(ContinueLifting::Continue(self.fallthrough), |_| ContinueLifting::Stop);
        Self::_finish(self.block_builder, self.start, self.fallthrough, self.branch_targets)?;
        self.block_builder.finish_block()?;
        self.block_builder.start_block()?;
        Ok(cont)
    }
}
