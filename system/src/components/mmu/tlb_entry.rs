use crate::components::mmu::Permission;
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};

// TODO: Rename this to something more appropriate?
#[derive(Copy, Clone, Debug)]
pub struct TLBEntry {
    /// The virtual address of the mapping
    pub virtual_address: EVAddr,
    /// The physical address the virtual address is mapped to
    pub physical_address: EPAddr,
    /// The permissions for the mapped address
    pub permissions: Permission,
    /// The size for which this mapping extends
    pub size: usize,
}

impl TLBEntry {
    pub fn new(
        virtual_address: EVAddr,
        physical_address: EPAddr,
        permissions: Permission,
        size: usize,
    ) -> Self {
        Self {
            virtual_address,
            physical_address,
            permissions,
            size,
        }
    }
}
