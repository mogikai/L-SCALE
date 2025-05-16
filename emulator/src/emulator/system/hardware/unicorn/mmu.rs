use crate::emulator::system::hardware::unicorn::UCWrapper;
use crate::emulator::system::hardware::unicorn::cpu::cpu_error_from_uc_error;
use crate::emulator::system::hardware::unicorn::hooks::mem_type_to_memory_action;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::components::mmu::tlb_entry::TLBEntry;
use mogi_system::components::mmu::{
    MMU,
    MemoryAction,
    MmuResult,
    PageFaultCallback,
    PageMetaInfo,
    Permission,
    TLB,
};
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
    HVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::trace;
use unicorn_engine::unicorn_const::{
    MemType,
    uc_error,
};
use unicorn_engine::{
    HookType,
    Unicorn,
    unicorn_const as uc_const,
};

#[derive(Debug)]
pub struct DummyData;
impl PageMetaInfo for DummyData {
    fn page_size() -> usize {
        todo!()
    }

    fn page_offset_bits() -> usize {
        todo!()
    }
}

impl MMU for UCWrapper<'_> {
    type MemoryContext = u64;
    type PageData = DummyData;

    fn new_context(&self) -> Self::MemoryContext {
        todo!()
    }

    fn clone_context(&self) -> Self::MemoryContext {
        todo!()
    }

    fn load_context(
        &mut self,
        context: &Self::MemoryContext,
    ) -> Self::MemoryContext {
        todo!()
    }

    fn flush_vaddr(
        &mut self,
        addr: EVAddr,
        len: usize,
    ) -> MmuResult<MemoryError> {
        todo!()
    }

    fn flush_all(&mut self) -> MmuResult<()> {
        self.uc_mut()
            .ctl_flush_tlb()
            .map_err(|_| MemoryError::Unknown)
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
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        self.uc_mut()
            .mem_read(addr.into(), buffer)
            .map_err(|_| MemoryError::Unknown)
    }

    fn write_physical(
        &mut self,
        addr: EPAddr,
        data: &[u8],
    ) -> MmuResult<()> {
        self.uc_mut()
            .mem_write(addr.into(), data)
            .map_err(|_| MemoryError::Unknown)
    }

    fn fetch_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        todo!()
    }

    fn read_bytes(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        todo!()
    }

    fn write_bytes(
        &mut self,
        addr: mogi_utilities::addresses::EVAddr,
        data: &[u8],
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
        size: usize,
    ) -> MmuResult<()> {
        Ok(self
            .uc_mut()
            .mem_map(addr.into(), size, unicorn_engine::Permission::all())
            .map_err(|_| MemoryError::Unknown)?)
    }

    fn free_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        todo!()
    }

    fn set_page_fault_callback(
        &mut self,
        mut callback: PageFaultCallback, // TODO
    ) -> MmuResult<()> {
        let trampoline = move |uc: &mut UCWrapper<'_>, addr, action| callback(addr, action);
        self.add_vtlb_hook(Box::new(trampoline));
        Ok(())
    }

    unsafe fn populate_tlb<T>(
        &self,
        tlb: &mut T,
        addr: EVAddr,
        action: MemoryAction,
    ) -> Result<HVAddr<Self::PageData>, MemoryError>
    where
        T: TLB<PageData = Self::PageData>,
    {
        todo!()
    }
}
