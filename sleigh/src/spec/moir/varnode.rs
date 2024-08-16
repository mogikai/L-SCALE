use crate::new_symbol_id_type;
use crate::spec::moir::address_space::MOIRSpaceId;
use crate::spec::moir::field::MOIRFieldId;
use crate::spec::moir::scope::MOIRTemporaryId;
use crate::spec::moir::table::MOIRTableId;

#[derive(Debug)]
/// A varnode is the unit of data manipulated by p-code.
/// It is simply a contiguous sequence of bytes in some address space.
///
/// Considering all the provided values, the storage location corresponding to a specific
/// varnode can be interpreted as: `space_addr + offset..space_addr+offset+size`
pub struct MOIRVarNode {
    // TODO: SpaceId
    /// The space can be interpreted as an address to the start of a memory region that
    /// is designated for storing certain values. For example, there may be a register
    /// space that is designated to saving the cpus registers when they are evicted from
    /// the host register.
    pub space: MOIRSpaceId,
    /// The offset specifies the offset from the start address of the space.
    pub offset: u64,
    /// The size specifies the size in bytes of the varnode.
    pub size: u16,
}

/// A slice of an existing varnode. The slice is at most as large as the original varnode.
#[derive(Debug, Copy, Clone)]
pub struct MOIRSlice {
    /// The slice refers to a symbol. This can be for example a varnode, a subtable export, or a field.
    pub reference: MOIRReference,
    /// The start byte of the slice
    pub start: u64,
    /// The end byte of the slice.
    pub end: u64,
}

impl MOIRSlice {
    pub fn varnode(
        varnode: MOIRVarNodeId,
        start: u64,
        end: u64,
    ) -> Self {
        Self {
            reference: MOIRReference::VarNode(varnode),
            start,
            end,
        }
    }

    pub fn temporary(
        temporary: MOIRTemporaryId,
        start: u64,
        end: u64,
    ) -> Self {
        Self {
            reference: MOIRReference::Temporary(temporary),
            start,
            end,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRReference {
    Field(MOIRFieldId),
    SubTable(MOIRTableId),
    VarNode(MOIRVarNodeId),
    Temporary(MOIRTemporaryId),
}

/// A value references either a constant, or a symbol that is later resolved to a const or a varnode.
#[derive(Debug, Copy, Clone)]
pub enum MOIRValue {
    /// The value references a slice of a symbol
    ValueSlice(MOIRSlice),
    /// The value is a constant value that can be used directly
    Const(MOIRConst),
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}

new_symbol_id_type! { pub struct MOIRVarNodeId; }
