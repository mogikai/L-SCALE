pub enum PCodeValue {
    Variable(PCodeVarNode),
    Const(PCodeConstant),
}

pub struct PCodeVarId {}

pub struct PCodeVarNode {
    id: PCodeVarId,
    /// The offset in variable specified by id
    offset: u16,
    /// The size of the variable. Maximum value with offset zero is the size of the original variable pointed to by id.
    size: u8,
}

pub enum PCodeConstant {
    Const8(u8),
    Const16(u16),
    Const32(u32),
    Const64(u64),
}
