use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::mmu::Permission;

#[derive(Copy, Clone)]
pub struct TLBEntry {
    pub virtual_address: EVAddr,
    pub physical_address: EPAddr,
    pub permissions: Permission,
}

impl TLBEntry {
    pub fn new(
        virtual_address: EVAddr,
        physical_address: EPAddr,
        permissions: Permission,
    ) -> Self {
        Self {
            virtual_address,
            physical_address,
            permissions,
        }
    }
}