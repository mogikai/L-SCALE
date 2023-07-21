use unicorn_engine::unicorn_const as uc_const;
use unicorn_engine::unicorn_const::{uc_error, MemType};

use crate::emulator::abstractions::cpu::address::EPAddr;
use crate::emulator::abstractions::mmu::{MemoryAction, MemoryError, Permission, MMU};
use crate::emulator::system::hardware::unicorn::UCWrapper;

impl MMU for UCWrapper<'_> {
    fn mem_map(
        &mut self,
        address: EPAddr,
        size: usize,
        perms: Permission,
    ) -> Result<(), MemoryError> {
        Ok(self
            .uc_mut()
            .mem_map(address.into(), size, perms.try_into()?)?)
    }

    fn mem_unmap(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> Result<(), MemoryError> {
        Ok(self.uc_mut().mem_unmap(address.into(), size)?)
    }

    fn mem_read_bytes(
        &self,
        address: EPAddr,
        len: usize,
    ) -> Result<Vec<u8>, MemoryError> {
        Ok(self.uc().mem_read_as_vec(address.into(), len)?)
    }

    fn mem_write_bytes(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> Result<usize, MemoryError> {
        self.uc_mut().mem_write(address.into(), data)?;
        Ok(data.len())
    }

    fn flush_tlb(&mut self) -> Result<(), MemoryError> {
        Ok(self.uc_mut().ctl_flush_tlb()?)
    }
}

impl From<MemType> for MemoryAction {
    fn from(memtype: MemType) -> Self {
        match memtype {
            MemType::READ => MemoryAction::READ,
            MemType::WRITE => MemoryAction::WRITE,
            MemType::FETCH => MemoryAction::FETCH,
            MemType::READ_UNMAPPED => MemoryAction::READ_UNMAPPED,
            MemType::WRITE_UNMAPPED => MemoryAction::WRITE_UNMAPPED,
            MemType::FETCH_UNMAPPED => MemoryAction::FETCH_UNMAPPED,
            MemType::WRITE_PROT => MemoryAction::WRITE_PROT,
            MemType::READ_PROT => MemoryAction::READ_PROT,
            MemType::FETCH_PROT => MemoryAction::FETCH_PROT,
            MemType::READ_AFTER => MemoryAction::READ_AFTER,
        }
    }
}

impl From<uc_error> for MemoryError {
    fn from(e: uc_error) -> Self {
        trace!("UC ERROR: {:?}", e);
        MemoryError::Unknown
    }
}

impl TryFrom<Permission> for uc_const::Permission {
    type Error = MemoryError;

    fn try_from(perm: Permission) -> Result<uc_const::Permission, MemoryError> {
        uc_const::Permission::from_bits(perm.bits()).ok_or(MemoryError::Unknown)
    }
}
