use crate::spec::ProcessorSpecResult;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::instruction::JIRTempId;
use crate::spec::moir::address_space::SpaceId;
pub use crate::spec::moir::varnode::VarNodeId;
use std::fmt::{
    Display,
    Formatter,
};

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct JIRSlice {
    /// Slices referring to the same varnode may be mapped to the same host register for optimization purposes
    /// I.e., it might make sense to only load the RAX varnode into a 64 bit register and then mark any
    /// subsequent accesses to e.g. EAX as already loaded. This depends on the host architecture.
    pub varnode_id: JIRVarNodeId,
    pub start: JIRSize,
    pub end: JIRSize,
}

impl PartialEq<JIRValue> for JIRSlice {
    fn eq(
        &self,
        other: &JIRValue,
    ) -> bool {
        match other {
            JIRValue::Slice(slice) => slice == self,
            JIRValue::Const(_) => false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum JIRVarNodeId {
    VarNode(VarNodeId),
    Temp(JIRTempId),
}

impl Display for JIRVarNodeId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            JIRVarNodeId::VarNode(varnode) => write!(f, "{}", varnode),
            JIRVarNodeId::Temp(temp) => write!(f, "{}", temp),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum JIRValue {
    Slice(JIRSlice),
    Const(JIRConst),
}

/// Compare slice against value. If one is not a slice, then this returns false.
/// If both refer to the same slice, they are considered equal.
impl PartialEq<JIRSlice> for JIRValue {
    fn eq(
        &self,
        other: &JIRSlice,
    ) -> bool {
        match self {
            JIRValue::Slice(slice) => slice == other,
            JIRValue::Const(_) => false,
        }
    }
}

impl JIRValue {
    pub fn size(&self) -> JIRSize {
        match self {
            JIRValue::Slice(slice) => slice.size(),
            JIRValue::Const(value) => value.size(),
        }
    }

    /// Unwraps the containing slice. If the [JIRValue] does not contain a slice, this will panic.
    pub fn unwrap_slice(self) -> JIRSlice {
        match self {
            JIRValue::Slice(slice) => slice,
            JIRValue::Const(_) => {
                panic!("Trying to unwrap a JIRConst into a slice.")
            }
        }
    }
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

    pub fn value(&self) -> u128 {
        self.value
    }

    pub fn size(&self) -> JIRSize {
        self.size
    }
}

impl JIRSlice {
    pub fn new(
        varnode_id: VarNodeId,
        start: JIRSize,
        end: JIRSize,
    ) -> Self {
        Self {
            varnode_id: JIRVarNodeId::VarNode(varnode_id),
            start,
            end,
        }
    }

    pub fn new_temp(
        temp_id: JIRTempId,
        start: JIRSize,
        end: JIRSize,
    ) -> Self {
        Self {
            varnode_id: JIRVarNodeId::Temp(temp_id),
            start,
            end,
        }
    }

    pub fn size(&self) -> JIRSize {
        self.end - self.start
    }
}

impl From<JIRSlice> for JIRValue {
    fn from(value: JIRSlice) -> Self {
        JIRValue::Slice(value)
    }
}

impl From<&JIRSlice> for JIRValue {
    fn from(value: &JIRSlice) -> Self {
        JIRValue::Slice(*value)
    }
}

impl From<JIRConst> for JIRValue {
    fn from(value: JIRConst) -> Self {
        JIRValue::Const(value)
    }
}

impl From<&JIRConst> for JIRValue {
    fn from(value: &JIRConst) -> Self {
        JIRValue::Const(*value)
    }
}

impl From<u64> for JIRValue {
    fn from(value: u64) -> Self {
        JIRConst::new(value as u128, 8).into()
    }
}

impl From<u32> for JIRValue {
    fn from(value: u32) -> Self {
        JIRConst::new(value as u128, 4).into()
    }
}

impl From<u16> for JIRValue {
    fn from(value: u16) -> Self {
        JIRConst::new(value as u128, 2).into()
    }
}

impl From<u8> for JIRValue {
    fn from(value: u8) -> Self {
        JIRConst::new(value as u128, 1).into()
    }
}

impl From<i64> for JIRValue {
    fn from(value: i64) -> Self {
        JIRConst::new(value as u128, 8).into()
    }
}

impl From<i32> for JIRValue {
    fn from(value: i32) -> Self {
        JIRConst::new(value as u128, 4).into()
    }
}

impl From<i16> for JIRValue {
    fn from(value: i16) -> Self {
        JIRConst::new(value as u128, 2).into()
    }
}

impl From<i8> for JIRValue {
    fn from(value: i8) -> Self {
        JIRConst::new(value as u128, 1).into()
    }
}

impl Display for JIRValue {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            JIRValue::Slice(slice) => write!(f, "{slice}"),
            JIRValue::Const(value) => write!(f, "{value}"),
        }
    }
}

impl Display for JIRSlice {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.varnode_id)
    }
}

impl Display for JIRConst {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:#0x}", self.value)
    }
}
