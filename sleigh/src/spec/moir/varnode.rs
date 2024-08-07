use crate::new_symbol_id_type;
use crate::spec::moir::address_space::MOIRSpaceId;

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
    space: MOIRSpaceId,
    /// The offset specifies the offset from the start address of the space.
    offset: u64,
    /// The size specifies the size in bytes of the varnode.
    size: u16,
}

new_symbol_id_type! { pub struct MOIRVarNodeId; }
