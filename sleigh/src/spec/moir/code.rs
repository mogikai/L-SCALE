use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRInstruction {
    /// The inputs of the instruction.
    inputs: Vec<MOIRValue>,
    /// The outputs of the instruction. The outputs must always be varnodes, since they are written to.
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
