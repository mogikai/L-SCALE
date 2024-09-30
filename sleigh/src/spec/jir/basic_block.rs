use crate::spec::jir::instruction::TargetInstruction;
use mogi_utilities::addresses::EVAddr;
use std::rc::Rc;

/// A target basic block contains multiple [TargetInstruction]s and also keeps track of
/// the predecessors and successor. Cloning this structure simply creates a new pointer
/// until the structure is accessed mutably, upon which the content is cloned.
#[derive(Debug, Clone)]
pub struct TargetBasicBlock(Rc<TargetBasicBlockInner>);

impl TargetBasicBlock {
    pub fn predecessors(&self) -> &Vec<EVAddr> {
        &self.0.predecessors
    }
    pub fn predecessors_mut(&mut self) -> &mut Vec<EVAddr> {
        &mut Rc::make_mut(&mut self.0).predecessors
    }

    pub fn instructions(&self) -> &Vec<TargetInstruction> {
        &self.0.instructions
    }

    pub fn push_instruction(
        &mut self,
        instruction: TargetInstruction,
    ) {
        Rc::make_mut(&mut self.0).instructions.push(instruction);
    }
}

#[derive(Debug, Clone)]
struct TargetBasicBlockInner {
    instructions: Vec<TargetInstruction>,
    /// A list of predecessor blocks that refer to this basic block.
    predecessors: Vec<EVAddr>,
    /// The successor of this basic block. Some blocks may not have any successor.
    /// This might be the case if the block issues some kind of halt instruction
    /// that stops execution.
    successor: Option<EVAddr>,
}
