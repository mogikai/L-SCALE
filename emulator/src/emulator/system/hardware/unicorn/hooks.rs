use unicorn_engine::unicorn_const::HookType;
use unicorn_engine::{InsnSysX86, TlbEntry, Unicorn};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MemoryAction;
use crate::emulator::system::hardware::unicorn::UCWrapper;
use crate::emulator::system::memory::TLBEntry;

impl<'a> Hooks<'a> for UCWrapper<'a> {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut() + 'a>,
    ) -> Result<(), CPUError> {
        // The trampoline takes ownership of the hook. The trampoline is then owned by the unicorn instance for as long
        // as the UCWrapper instance is alive.
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>| {
            hook();
        };
        // TODO: Store hook id so we can remove it?
        let _hook = self
            .uc_mut()
            .add_insn_sys_hook(InsnSysX86::SYSCALL, start.into(), end.into(), trampoline)?;
        Ok(())
    }

    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, MemoryAction, u64, usize, i64) -> bool + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, mem_type, address, size, value| {
            hook(unsafe { &mut **uc.get_data_mut() }, MemoryAction::from(mem_type), address, size, value)
        };
        // TODO: Store hook id to be able to remove hook later on
        let _hook = self
            .uc_mut()
            .add_mem_hook(HookType::MEM_UNMAPPED, start.into(), end.into(), trampoline)?;
        Ok(())
    }

    fn add_block_hook(
        &mut self,
        mut hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline =
            move |uc: &mut Unicorn<*mut UCWrapper<'a>>, a, b| hook(unsafe { &mut **uc.get_data_mut() }, a, b);
        // TODO: Store hook id so we can remove it later
        let _hook = self.uc_mut().add_block_hook(trampoline)?;
        Ok(())
    }

    fn add_vtlb_hook(
        &mut self,
        mut hook: Box<dyn FnMut(&mut Self, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, address: u64, access_type| {
            hook(unsafe { &mut **uc.get_data_mut() }, address.into(), MemoryAction::from(access_type))
                .map(|tlbentry| tlbentry.into())
        };
        self.uc_mut().add_tlb_hook(1, 0, trampoline)?;
        Ok(())
    }

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, address, size| {
            hook(unsafe { &mut **uc.get_data_mut() }, address, size)
        };
        let _hook = self
            .uc_mut()
            .add_code_hook(start.into(), end.into(), trampoline)?;
        Ok(())
    }
}

impl From<TLBEntry> for TlbEntry {
    fn from(value: TLBEntry) -> Self {
        TlbEntry {
            paddr: value.physical_address.into(),
            perms: value.permissions.try_into().unwrap(),
        }
    }
}
