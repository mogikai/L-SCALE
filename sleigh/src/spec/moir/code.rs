use crate::spec::moir::varnode::MOIRVarNodeId;

#[derive(Debug)]
pub struct MOIRInstruction {
    /// The first input of the instruction.
    pub input_a: MOIRValue,
    pub input_b: MOIRValue,
    /// The output of the instruction. The output must always be a varnode, since it is written to.
    pub output: MOIRVarNodeId,
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRValue {
    VarNode(MOIRVarNodeId),
    Const(MOIRConst),
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}

pub enum MOIRInstructionType {
    CustomOp(Vec<MOIRValue>),
}
