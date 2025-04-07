use crate::core::Exception;
use crate::cpu::api::CpuError;
use crate::cpu::exception::ExceptionCode;
use crate::cpu::exit::CpuExitStatus;
use crate::cpu::{
    Cpu,
    Snapshot,
};
use mogi_mem_legacy::mmu::MemoryContext;
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
    MemoryAction,
    MmuResult,
    PageFaultCallback,
    PageMetaInfo,
    Permission,
    MMU,
    TLB,
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
    HVAddr,
};
use mogi_utilities::error::memory::MemoryError;
use std::time::Duration;

pub struct DummyData;
impl PageMetaInfo for DummyData {
    fn page_size() -> usize {
        todo!()
    }

    fn page_offset_bits() -> usize {
        todo!()
    }
}

impl MMU for Cpu {
    type MemoryContext = MemoryContext;
    type PageData = DummyData;

    fn new_context(&self) -> Self::MemoryContext {
        self.core.mem.borrow().new_context()
    }

    fn clone_context(&self) -> Self::MemoryContext {
        self.core.mem.borrow().clone_context()
    }

    fn load_context(
        &mut self,
        context: &Self::MemoryContext,
    ) -> Self::MemoryContext {
        self.core.mem.borrow_mut().load_context(context)
    }

    fn flush_vaddr(
        &mut self,
        addr: EVAddr,
        len: usize,
    ) -> MmuResult<MemoryError> {
        todo!()
    }

    fn flush_all(&mut self) -> MmuResult<()> {
        self.core.mem.borrow_mut().flush_all()
    }

    fn set_page_table_entry(
        &mut self,
        addr: EVAddr,
        paddr: EPAddr,
        permissions: Permission,
    ) -> MmuResult<()> {
        self.core
            .mem
            .borrow_mut()
            .set_page_table_entry(addr, paddr, permissions)
    }

    fn clear_page_table_entry(
        &mut self,
        addr: EVAddr,
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().clear_page_table_entry(addr)
    }

    fn read_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().read_physical(addr, buffer)
    }

    fn write_physical(
        &mut self,
        addr: EPAddr,
        data: &[u8],
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().write_physical(addr, data)
    }

    fn fetch_physical(
        &mut self,
        addr: EPAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.core.mem.borrow_mut().fetch_physical(addr, buffer)
    }

    fn read(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().read(addr, buffer)
    }

    fn write(
        &mut self,
        addr: EVAddr,
        data: &[u8],
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().write(addr, data)
    }

    fn fetch(
        &mut self,
        addr: EVAddr,
        buffer: &mut [u8],
    ) -> Result<(), MemoryError> {
        self.core.mem.borrow_mut().fetch(addr, buffer)
    }

    fn alloc_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().alloc_physical(addr, size)
    }

    fn free_physical(
        &mut self,
        addr: EPAddr,
        size: usize,
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().free_physical(addr, size)
    }

    fn set_page_fault_callback(
        &mut self,
        callback: PageFaultCallback,
    ) -> MmuResult<()> {
        self.core.mem.borrow_mut().set_page_fault_callback(callback)
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
            CpuExitStatus::Stopped(reason) => Err(CPUError::Stopped(reason)),
        }
    }

    fn stop(
        &mut self,
        stop_reason: StopReason,
    ) -> Result<(), CPUError> {
        self.core.stop_reason = Some(stop_reason);
        Ok(())
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
                cpu.stop_reason = None;
                syscall();
                if let Some(stop_reason) = cpu.stop_reason {
                    Some(CpuExitStatus::Stopped(stop_reason))
                } else {
                    cpu.resume_next();
                    let pc = cpu.read_pc();
                    cpu.exception = Exception::new(ExceptionCode::ExternalAddr, pc.into());
                    None
                }
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
                    hook(&mut *slfptr, addr, action);
                    None
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
        self.snapshots.push(Snapshot {
            core: self.core.snapshot(),
        });
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        let snapshot = self.snapshots.last().ok_or(GenericError::unknown())?;
        self.core.restore(&snapshot.core);
        self.update_context();

        tracing::trace!(
            "VM state restored: pc = {:?}, block.id={}, block.offset={}",
            self.core.read_pc(),
            self.core.block_id,
            self.core.block_offset
        );
        Ok(())
    }
}
