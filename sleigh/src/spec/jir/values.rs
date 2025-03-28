use crate::spec::ProcessorSpecificationError;
use crate::spec::jir::basic_block::instruction::JIRTempId;
use crate::spec::jir::common::{
    JIROffset,
    JIRSize,
};
use crate::spec::moir::address_space::SpaceId;
pub use crate::spec::moir::context::ContextId;
use crate::spec::moir::varnode::MOIRVarNode;
pub use crate::spec::moir::varnode::VarNodeId;
use mogi_sleigh_parser::StringId;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use mogi_utilities::overlap::overlaps_generic;
use mogi_utilities::{
    new_map_key,
    new_map_key_8,
};
use num::complex::ComplexFloat;
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::mem::take;
use std::ops::Range;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

// Varnodes are compacted on transformation, but overlapping varnodes need to keep
// their overlap. We group them into into intersection groups which we then compact.
#[derive(Serialize, Deserialize)]
pub(crate) struct VarnodeGroup {
    pub(crate) varnodes: Vec<VarNodeId>,
    // track the total size of the group
    pub(crate) offset: u64,
    pub(crate) size: JIRSize,
}

impl Debug for VarnodeGroup {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("VarnodeGroup")
            .field("varnodes", &format_args!("{} VIds", self.varnodes.len()))
            .field("offset", &self.offset)
            .field("end", &self.end())
            .finish()
    }
}

impl From<&VarnodeGroup> for Range<u64> {
    fn from(value: &VarnodeGroup) -> Self {
        value.offset..value.end()
    }
}

impl From<&mut VarnodeGroup> for Range<u64> {
    fn from(value: &mut VarnodeGroup) -> Self {
        value.offset..value.end()
    }
}

impl VarnodeGroup {
    pub(crate) fn new(
        varnodes: Vec<VarNodeId>,
        offset: u64,
        size: JIRSize,
    ) -> Self {
        Self { varnodes, offset, size }
    }

    pub fn end(&self) -> u64 {
        self.offset + u64::from(self.size)
    }

    /// Expands the group such that it contains the provided varnode
    pub fn insert(
        &mut self,
        varnode_id: VarNodeId,
        varnode: &MOIRVarNode,
    ) {
        self.varnodes.push(varnode_id);
        // Adjust offset if necessary.
        self.offset = self.offset.min(varnode.offset);
        // Calculate the new end
        let new_end = self.end().max(varnode.end());
        // And then set the size accordingly
        self.size = JIRSize::try_from(new_end - self.offset).expect("This cannot overflow");
    }

    pub fn merge(
        &mut self,
        other: Self,
    ) {
        self.offset = self.offset.min(other.offset);
        let new_end = self.end().max(other.end());
        self.size = JIRSize::try_from(new_end - self.offset).expect("This cannot overflow");
        self.varnodes.extend(other.varnodes);
    }
}

new_map_key_8! { pub struct VarnodeGroupId; "VGI"; }

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct JIRSlice {
    /// The space this slice is located in.
    pub space_id: SpaceId,
    /// The group of varnodes this slice belongs to. Temps do not have a group
    pub group_id: Option<VarnodeGroupId>,
    /// The offset in the space where this slice is located.
    pub offset: JIROffset,
    /// The size of the slice in bytes.
    pub size: JIRSize,
}

impl Debug for JIRSlice {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if let Some(group_id) = self.group_id {
            write!(f, "{}::<{}>[{:#x}..{:#x}]", group_id, self.space_id, self.offset, self.end())
        } else {
            write!(f, "TMP::<{}>[{:#x}..{:#x}]", self.space_id, self.offset, self.end())
        }
    }
}

impl From<JIRSlice> for Range<JIROffset> {
    fn from(value: JIRSlice) -> Self {
        value.offset..value.end()
    }
}

impl From<&JIRSlice> for Range<JIROffset> {
    fn from(value: &JIRSlice) -> Self {
        value.offset..value.end()
    }
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    pub fn is_slice_then<F>(
        &self,
        f: F,
    ) where
        F: FnOnce(&JIRSlice),
    {
        if let JIRValue::Slice(slice) = self {
            f(slice)
        }
    }

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

    pub fn slice(
        self,
        start: JIROffset,
        size: JIRSize,
    ) -> JIRValue {
        match self {
            JIRValue::Slice(slice) => slice.slice(start, size).into(),
            JIRValue::Const(value) => value.slice(start, size).into(),
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

    pub fn value_mut(&mut self) -> &mut u128 {
        &mut self.value
    }

    pub fn value(&self) -> u128 {
        self.value
    }

    pub fn size(&self) -> JIRSize {
        self.size
    }

    pub fn slice(
        self,
        offset: JIROffset,
        size: JIRSize,
    ) -> JIRConst {
        todo!()
    }

    pub fn zext(
        self,
        new_size: JIRSize,
    ) -> Option<JIRConst> {
        if self.size <= new_size {
            Some(JIRConst::new(self.value, new_size))
        } else {
            None
        }
    }
}

impl JIRSlice {
    pub fn new(
        space_id: SpaceId,
        group_id: VarnodeGroupId,
        offset: JIROffset,
        size: JIRSize,
    ) -> Self {
        Self {
            space_id,
            group_id: Some(group_id),
            offset,
            size,
        }
    }

    pub fn new_temp(
        space_id: SpaceId,
        offset: JIROffset,
        size: JIRSize,
    ) -> Self {
        Self {
            space_id,
            group_id: None,
            offset,
            size,
        }
    }

    #[inline]
    pub fn slice(
        self,
        start: JIROffset,
        size: JIRSize,
    ) -> JIRSlice {
        if self.end() < start {
            todo!()
        }
        Self {
            space_id: self.space_id,
            group_id: self.group_id,
            offset: self.offset + JIROffset::from(start),
            size,
        }
    }

    /// Checks whether `self` overlaps with `other`.
    #[inline]
    pub fn overlaps(
        &self,
        other: &JIRSlice,
    ) -> bool {
        overlaps_generic(self, other)
    }

    #[inline]
    pub fn size(&self) -> JIRSize {
        self.size
    }

    #[inline]
    pub fn end(&self) -> JIROffset {
        self.offset + JIROffset::from(self.size)
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

impl TryFrom<JIRConst> for EVAddr {
    type Error = ProcessorSpecificationError;

    fn try_from(value: JIRConst) -> Result<Self, Self::Error> {
        Ok(EVAddr::from(u64::try_from(value.value).map_err(|_| -> Self::Error { todo!() })?))
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
        write!(f, "{}[{}..={}]", self.space_id, self.offset, self.size)
    }
}

impl Display for JIRConst {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:#0x}:{}", self.value, self.size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn size_of_slice_is_less_than_64_bits() {
        assert!(std::mem::size_of::<JIRSlice>() <= 8);
    }
}
