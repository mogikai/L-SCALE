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
pub struct JIRConst {
    value: u128,
    size: JIRSize,
}

impl JIRConst {
    pub fn new(
        value: u128,
        size: JIRSize,
    ) -> Self {
        Self { value, size }
    }
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

impl From<JIRConst> for JIRValue {
    fn from(value: JIRConst) -> Self {
        JIRValue::Const(value)
    }
}
