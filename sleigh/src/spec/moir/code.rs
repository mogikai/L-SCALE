use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRInstruction {
    inputs: Vec<MOIRValue>,
    outputs: Vec<MOIRVarNodeId>,
}

#[derive(Debug)]
pub enum MOIRValue {
    VarNode(MOIRVarNodeId),
    Const(MOIRConst),
}

#[derive(Debug)]
pub enum MOIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}

pub enum MOIRInstructionType {}
