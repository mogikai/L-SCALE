use crate::spec::MOIRVarNodeId;

#[derive(Debug, Default)]
pub struct MOIRMacro {
    pub arguments: Vec<MOIRVarNodeId>,
    pub code: Vec<MOIRInstruction>,
}

#[derive(Debug)]
pub struct MOIRInstruction {}
