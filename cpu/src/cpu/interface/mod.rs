use crate::cpu::Cpu;
use mogi_mem::perm::{
    EXEC,
    NONE,
    READ,
    WRITE,
};
use mogi_mem::{
    AllocLayout,
    MemoryMapping,
    UnallocatedMemory,
};
use mogi_system::components::cpu::{
    ContextHandle,
    Register,
    CPU,
};
use mogi_system::components::mmu::{
    MmuResult,
    Permission,
    MMU,
};
use mogi_system::errors::{
    CPUError,
    StopReason,
};
use mogi_utilities::addresses::{
    EPAddr,
    EVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::time::Duration;

impl MMU for Cpu {
    fn alloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
        perms: Permission,
    ) -> MmuResult<()> {
        let addr = self
            .core
            .mem
            .borrow_mut()
            .alloc_memory(
                AllocLayout {
                    addr: Some(address.into()),
                    size: size as u64,
                    align: 0,
                },
                MemoryMapping::Unallocated(UnallocatedMemory {
                    perm: permission_to_mogi_permission(perms),
                    value: 0,
                }),
            )
            .map_err(|_| MemoryError::Unknown)?;
        assert_eq!(addr, address.into());
        Ok(())
    }

    fn dealloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        todo!()
    }

    fn read_physical(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>> {
        todo!()
    }

    fn write_physical(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize> {
        todo!()
    }

    fn flush_tlb(&mut self) -> MmuResult<()> {
        todo!()
    }
}

fn permission_to_mogi_permission(perm: Permission) -> u8 {
    let mut result = NONE;
    if perm.contains(Permission::READ) {
        result |= READ;
    }
    if perm.contains(Permission::WRITE) {
        result |= WRITE;
    }
    if perm.contains(Permission::EXEC) {
        result |= EXEC;
    }
    result
}

impl CPU for Cpu {
    fn emulate(
        &mut self,
        stop: Option<EVAddr>,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn stop(
        &mut self,
        stop_reason: StopReason,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn reg_write(
        &mut self,
        register: Register,
        value: u64,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn reg_read(
        &self,
        register: Register,
    ) -> Result<u64, CPUError> {
        todo!()
    }

    fn add_breakpoint(
        &mut self,
        address: EVAddr,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn remove_breakpoint(
        &mut self,
        address: EVAddr,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn create_context(&mut self) -> Result<ContextHandle, CPUError> {
        todo!()
    }

    fn save_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn restore_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn remove_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        todo!()
    }
}
