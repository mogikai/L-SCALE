use crate::spec::moir::varnode::VarNodeId;

#[derive(Debug)]
pub struct JIRInstruction {
    output: Option<VarNodeId>,
    input_a: Option<VarNodeId>,
    input_b: Option<VarNodeId>,
    operation: JIROperation,
}

#[derive(Debug)]
pub enum JIROperation {}
