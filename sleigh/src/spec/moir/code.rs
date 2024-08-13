use crate::spec::moir::varnode::{
    MOIRValue,
    MOIRVarNodeId,
};

#[derive(Debug)]
pub struct MOIRInstruction {
    /// The first input of the instruction.
    pub input_a: MOIRValue,
    pub input_b: MOIRValue,
    /// The output of the instruction. The output must always be a varnode, since it is written to.
    pub output: MOIRVarNodeId,
}

pub enum MOIRInstructionType {
    CustomOp(Vec<MOIRValue>),
}
