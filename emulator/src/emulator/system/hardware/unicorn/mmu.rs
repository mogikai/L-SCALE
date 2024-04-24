use mogi_system::components::mmu::{
    MemoryAction, MmuResult, PageFaultCallback, Permission, MMU
};
use mogi_utilities::addresses::{EPAddr, EVAddr};
use mogi_utilities::error::memory::MemoryError;
use tracing::trace;
use unicorn_engine::unicorn_const as uc_const;
use unicorn_engine::unicorn_const::{
    uc_error,
    MemType,
};

use crate::emulator::system::hardware::unicorn::UCWrapper;

impl MMU for UCWrapper<'_> {
    type MemoryContext = u64; // TODO

    fn new_context(
        &self
    ) -> Self::MemoryContext {
        todo!()
    }

    fn clone_context(
        &self
    ) -> Self::MemoryContext {
        todo!()
    }

    fn load_context(
        &mut self,
        context: &Self::MemoryContext
    ) -> Self::MemoryContext {
        todo!()
    }

    fn flush_vaddr(
        &mut self,
        addr: EVAddr,
        len: usize
    ) -> MmuResult<MemoryError> {
        todo!()
    }

    fn flush_all(
        &mut self
    ) -> MmuResult<()> {
        todo!()
    }

    fn set_page_table_entry(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        paddr: EPAddr,
        permissions: Permission,
    ) -> MmuResult<()> {
        todo!()
    }

    fn clear_page_table_entry(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
    ) -> MmuResult<()> {
        todo!()
    }

    fn read_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8]
    ) -> MmuResult<()> {
        todo!()
    }

    fn write_physical(
        &mut self,
        addr: EPAddr,
        data: &[u8]
    ) -> MmuResult<()> {
        todo!()
    }

    fn fetch_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        todo!()
    }

    fn read(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        buffer: &mut [u8]
    ) -> MmuResult<()> {
        todo!()
    }

    fn write(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        data: &[u8]
    ) -> MmuResult<()> {
        todo!()
    }

    fn fetch(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        todo!()
    }

    fn alloc_physical(
        &mut self,
        addr: EPAddr,
        size: usize
    ) -> MmuResult<()> {
        todo!()
    }

    fn free_physical(
        &mut self,
        addr: EPAddr,
        size: usize
    ) -> MmuResult<()> {
        todo!()
    }

    fn set_page_fault_callback(
        &mut self,
        callback: PageFaultCallback<u64> // TODO
    ) -> MmuResult<()> {
        todo!()
    }
}
