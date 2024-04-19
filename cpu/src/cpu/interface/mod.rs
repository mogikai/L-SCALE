use crate::core::Exception;
use crate::cpu::exception::ExceptionCode;
use crate::cpu::exit::CpuExitStatus;
use crate::cpu::Cpu;
use mogi_system::components::cpu::hooks::{
    BlockHook,
    CodeHook,
    Hooks,
    UnmappedMemHook,
    VtlbHook,
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
use mogi_system::components::restore::Restorable;
use mogi_system::errors::cpu::{
    CPUError,
    StopReason,
};
use mogi_system::errors::generic::GenericError;
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
        let _ = self
            .core
            .mem
            .borrow_mut()
            .physical_memory_mut()
            .alloc(address, size, perms)?;
        Ok(())
    }

    fn dealloc_physical(
        &mut self,
        address: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        self.core
            .mem
            .borrow_mut()
            .physical_memory_mut()
            .free(address, size)
    }

    fn read_physical(
        &self,
        address: EPAddr,
        len: usize,
    ) -> MmuResult<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        unsafe {
            out.set_len(len);
        }
        self.core
            .mem
            .borrow_mut()
            .read_physical(address.into(), out.as_mut())?;
        Ok(out)
    }

    fn write_physical(
        &mut self,
        address: EPAddr,
        data: &[u8],
    ) -> MmuResult<usize> {
        self.core
            .mem
            .borrow_mut()
            .write_physical(address.into(), data)?;
        Ok(data.len())
    }

    fn flush_tlb(&mut self) -> MmuResult<()> {
        self.core.mem.borrow_mut().flush();
        Ok(())
    }
}

impl CPU for Cpu {
    fn emulate(
        &mut self,
        stop: Option<EVAddr>,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), CPUError> {
        if let Some(instruction_count) = instruction_count {
            self.icount_limit = instruction_count as u64;
        }
        let status = self.run();
        match status {
            CpuExitStatus::Running => Err(CPUError::Stopped(StopReason::ProcessExit(0))),
            CpuExitStatus::InstructionLimit => Err(CPUError::Stopped(StopReason::Unknown)),
            CpuExitStatus::Breakpoint => Err(CPUError::Stopped(StopReason::Breakpoint)),
            CpuExitStatus::Interrupted => Err(CPUError::Stopped(StopReason::Unknown)),
            CpuExitStatus::Halt => Err(CPUError::Stopped(StopReason::ProcessExit(0))),
            CpuExitStatus::Killed => Err(CPUError::Stopped(StopReason::Killed(0))),
            CpuExitStatus::Deadlock => Err(CPUError::Stopped(StopReason::Unknown)),
            CpuExitStatus::OutOfMemory => Err(CPUError::Stopped(StopReason::Unknown)),
            CpuExitStatus::Unimplemented => Err(CPUError::Stopped(StopReason::Unknown)),
            CpuExitStatus::UnhandledException(_) => Err(CPUError::Unknown),
        }
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
        if let Register::X86(register) = register {
            self.write_reg(register.to_string().as_str(), value)
                .map_err(|_| CPUError::Unknown)
        } else {
            todo!("Only x86 currently supported");
        }
    }

    fn reg_read(
        &self,
        register: Register,
    ) -> Result<u64, CPUError> {
        if let Register::X86(register) = register {
            self.read_reg(register.to_string().as_str())
                .map_err(|_| CPUError::Unknown)
        } else {
            todo!("Only x86 currently supported");
        }
    }

    fn add_breakpoint(
        &mut self,
        address: EVAddr,
    ) -> Result<(), CPUError> {
        Ok(())
    }

    fn remove_breakpoint(
        &mut self,
        address: EVAddr,
    ) -> Result<(), CPUError> {
        todo!()
    }

    fn create_context(&mut self) -> Result<ContextHandle, CPUError> {
        Ok(0)
    }

    fn save_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        Ok(())
    }

    fn restore_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        Ok(())
    }

    fn remove_context(
        &mut self,
        context_handle: ContextHandle,
    ) -> Result<(), CPUError> {
        todo!()
    }
}

impl Hooks for Cpu {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut syscall: Box<dyn FnMut() + 'static>,
    ) -> Result<(), CPUError> {
        self.handle_exception_hook = Box::new(move |cpu| match ExceptionCode::from_u32(cpu.exception.code) {
            ExceptionCode::Syscall => {
                syscall();
                cpu.resume_next();
                let pc = cpu.read_pc();
                cpu.exception = Exception::new(ExceptionCode::ExternalAddr, pc.into());
                None
            }
            _ => None,
        });
        Ok(())
    }

    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: UnmappedMemHook<Self>,
    ) -> Result<(), CPUError> {
        Ok(())
    }

    fn add_block_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: BlockHook<Self>,
    ) -> Result<(), CPUError> {
        Ok(())
    }

    fn add_vtlb_hook(
        &mut self,
        mut hook: VtlbHook<Self>,
    ) -> Result<(), CPUError> {
        let slfptr: *mut Cpu = self;
        // Remove this fugly hack
        unsafe {
            self.core
                .mem
                .borrow_mut()
                .set_page_fault_callback(Box::new(move |addr, action| {
                    hook(&mut *slfptr, addr, action)
                        .ok_or(MemoryError::NotMapped)
                        .map(|e| e.physical_address)
                }));
        }
        Ok(())
    }

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: CodeHook<Self>,
    ) -> Result<(), CPUError> {
        todo!()
    }
}

impl Restorable for Cpu {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
