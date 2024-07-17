pub mod address_space;
pub mod varnode;

use crate::spec::address_space::AddressSpace;
use crate::spec::varnode::VarNode;
use slotmap::{
    new_key_type,
    SlotMap,
};

new_key_type! { pub struct AddressSpaceId; }
new_key_type! { pub struct VarNodeId; }

#[derive(Debug)]
pub struct ProcessorSpecification {
    address_spaces: SlotMap<AddressSpaceId, AddressSpace>,
    varnodes: SlotMap<VarNodeId, VarNode>,
}
