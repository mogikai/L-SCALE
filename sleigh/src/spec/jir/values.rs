use crate::spec::jir::common::JIRSize;
use crate::spec::moir::address_space::SpaceId;
pub use crate::spec::moir::varnode::VarNodeId;

#[derive(Debug)]
pub struct JIRVarNode {
    pub space_id: SpaceId,
    pub offset: u64,
    pub size: JIRSize,
}

impl JIRVarNode {
    pub fn end(&self) -> u64 {
        self.offset + self.size as u64
    }
}

#[derive(Debug, Copy, Clone)]
pub struct JIRSlice {
    pub varnode_id: VarNodeId,
    pub start: JIRSize,
    pub end: JIRSize,
}

#[derive(Debug, Copy, Clone)]
pub enum JIRValue {
    Slice(JIRSlice),
    Const(JIRConst),
}

#[derive(Debug, Copy, Clone)]
pub enum JIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}

impl JIRSlice {
    pub fn new(
        varnode_id: VarNodeId,
        start: JIRSize,
        end: JIRSize,
    ) -> Self {
        Self { varnode_id, start, end }
    }
}

impl From<JIRSlice> for JIRValue {
    fn from(value: JIRSlice) -> Self {
        JIRValue::Slice(value)
    }
}
