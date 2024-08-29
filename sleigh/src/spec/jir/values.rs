use crate::spec::jir::common::JIRSize;
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::varnode::VarNodeId;

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

pub struct JIRSlice {
    pub varnode_id: VarNodeId,
    pub start: JIRSize,
    pub end: JIRSize,
}

pub enum JIRValue {
    Slice(JIRSlice),
    Const(JIRConst),
}

pub enum JIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}
