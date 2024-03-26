use mogi_system::traits::mmu::{
    MemoryAction,
    MmuResult,
    Permission,
    MMU,
};
use mogi_utilities::addresses::EPAddr;
use mogi_utilities::error::memory::MemoryError;
use tracing::trace;
use unicorn_engine::unicorn_const as uc_const;
use unicorn_engine::unicorn_const::{
    uc_error,
    MemType,
};

use crate::emulator::system::hardware::unicorn::UCWrapper;

impl MMU for UCWrapper<'_> {
    fn alloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
        perms: Permission,
    ) -> MmuResult<()> {
        Ok(self
            .uc_mut()
            .mem_map(address.into(), size, uc_const::Permission::from_bits(perms.bits()).ok_or(MemoryError::Unknown)?)
            .map_err(|_| MemoryError::Unknown)?)
    }

    fn dealloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        Ok(self
            .uc_mut()
            .mem_unmap(address.into(), size)
            .map_err(|_| MemoryError::Unknown)?)
    }

    fn read_physical(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>> {
        Ok(self
            .uc()
            .mem_read_as_vec(address.into(), len)
            .map_err(|_| MemoryError::Unknown)?)
    }

    fn write_physical(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize> {
        self.uc_mut()
            .mem_write(address.into(), data)
            .map_err(|_| MemoryError::Unknown)?;
        Ok(data.len())
    }

    fn flush_tlb(&mut self) -> Result<(), MemoryError> {
        Ok(self
            .uc_mut()
            .ctl_flush_tlb()
            .map_err(|_| MemoryError::Unknown)?)
    }
}
