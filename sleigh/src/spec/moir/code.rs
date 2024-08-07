use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRInstruction {
    inputs: Vec<MOIRVarNodeId>,
    outputs: Vec<MOIRVarNodeId>,
}

pub enum MOIRInstructionType {}
