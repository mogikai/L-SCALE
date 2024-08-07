use crate::new_symbol_id_type;

#[derive(Debug)]
/// A varnode is the unit of data manipulated by p-code.
/// It is simply a contiguous sequence of bytes in some address space.
/// In contrast to the sleigh specification, we distinguish between
/// `Const` and `Var` varnodes. This makes it easier to handle
/// const values, instead of having a separate `const` space to
/// which the varnodes refer.
pub enum MOIRVarNode {
    /// Considering all the provided values, the storage location corresponding to a specific
    /// varnode can be interpreted as: `space_addr + offset..space_addr+offset+size`
    Var {
        // TODO: SpaceId
        /// The space can be interpreted as an address to the start of a memory region that
        /// is designated for storing certain values. For example, there may be a register
        /// space that is designated to saving the cpus registers when they are evicted from
        /// the host register.
        space: (),
        /// The offset specifies the offset from the start address of the space.
        offset: u64,
        /// The size specifies the size in bytes of the varnode.
        size: u16,
    },
    /// Const values are directly stored in the varnode since they cannot change, and will
    /// be baked into the jited code anyway.
    Const(MOIRConst),
}

#[derive(Debug)]
pub enum MOIRConst {
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
}

new_symbol_id_type! { pub struct MOIRVarNodeId; }
