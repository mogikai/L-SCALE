use crate::spec::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRMacro {
    pub arguments: Vec<MOIRVarNodeId>,
    pub code: Vec<MOIRInstruction>,
}

#[derive(Debug)]
pub struct MOIRInstruction {}
