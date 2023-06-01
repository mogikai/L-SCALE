use std::cell::RefCell;
use std::rc::Rc;

use unicorn_engine::unicorn_const::HookType;
use unicorn_engine::{InsnSysX86, TlbEntry, Unicorn};

use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::{MemoryAction, MemoryError, Permission};
use crate::emulator::system::hardware::unicorn::UCWrapper;
use crate::emulator::system::memory::tlb::tlb_entry::TLBEntry;

impl<'a> Hooks<'a> for UCWrapper<'a> {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self) + 'a>,
    ) {
        // The trampoline takes ownership of the hook. The trampoline is then owned by the unicorn instance for as long
        // as the UCWrapper instance is alive.
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>| {
            // This is safe because the lifetime of unicorn and UCWrapper have a  1:1 relationship
            // Mutable access is also fine, since the unicorn callback is expected
            // to mutate stuff in the unicorn instance during callbacks.
            // The only other mutable access can come from UCWrapper itself
            let data = unsafe { &mut **uc.get_data_mut() };
            hook(data);
        };
        let _hook = self
            .uc_mut()
            .add_insn_sys_hook(InsnSysX86::SYSCALL, start.into(), end.into(), trampoline)
            .expect("failed to add syscall hook");
    }

    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, MemoryAction, u64, usize, i64) -> bool + 'a>,
    ) {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, mem_type, address, size, value| {
            hook(unsafe { &mut **uc.get_data_mut() }, MemoryAction::from(mem_type), address, size, value)
        };
        let _hook = self
            .uc_mut()
            .add_mem_hook(HookType::MEM_UNMAPPED, start.into(), end.into(), trampoline)
            .unwrap();
    }

    fn add_block_hook(
        &mut self,
        mut hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    ) {
        let trampoline =
            move |uc: &mut Unicorn<*mut UCWrapper<'a>>, a, b| hook(unsafe { &mut **uc.get_data_mut() }, a, b);
        let _hook = self.uc_mut().add_block_hook(trampoline);
    }

    fn add_vtlb_hook(
        &mut self,
        mut hook: Box<dyn FnMut(&mut Self, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>,
    ) -> Result<(), GenericError> {
        let trampoline = move |uc: &mut Unicorn<*mut UCWrapper<'a>>, address: u64, access_type| {
            hook(unsafe { &mut **uc.get_data_mut() }, address.into(), MemoryAction::from(access_type))
                .map(|tlbentry| tlbentry.into())
        };
        self.uc_mut().add_tlb_hook(1, 0, trampoline)?;
        Ok(())
    }
}

impl From<TLBEntry> for unicorn_engine::TlbEntry {
    fn from(value: TLBEntry) -> Self {
        TlbEntry {
            paddr: value.physical_address.into(),
            perms: value.permissions.try_into().unwrap(),
        }
    }
}
