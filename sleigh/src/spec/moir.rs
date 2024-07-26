use crate::spec::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRMacro {
    arguments: Vec<MOIRVarNodeId>,
    code: Vec<MOIRInstruction>,
}

#[derive(Debug)]
pub struct MOIRInstruction {}
