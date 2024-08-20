use crate::new_typed_index_table_id;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::space::JIRSpaceId;

pub struct JIRVarNode {
    pub space_id: JIRSpaceId,
    pub offset: u64,
    pub size: JIRSize,
}

new_typed_index_table_id! { pub struct JIRVarNodeId; }

pub struct JIRSlice {
    pub varnode_id: JIRVarNodeId,
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
