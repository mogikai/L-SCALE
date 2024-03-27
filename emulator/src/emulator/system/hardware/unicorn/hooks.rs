use mogi_system::components::mmu::tlb_entry::TLBEntry;
use mogi_system::components::mmu::MemoryAction;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::error::memory::MemoryError;
use unicorn_engine::unicorn_const::HookType;
use unicorn_engine::{
    InsnSysX86,
    MemType,
    Permission,
    TlbEntry,
    Unicorn,
};

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::system::hardware::unicorn::cpu::cpu_error_from_uc_error;
use crate::emulator::system::hardware::unicorn::UCWrapper;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::errors::CPUError;

impl<'a> Hooks for UCWrapper<'a> {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut() + 'a>,
    ) -> Result<(), CPUError> {
        // The trampoline takes ownership of the hook. The trampoline is then owned by the unicorn instance for as long
        // as the UCWrapper instance is alive.
        let trampoline = move |uc: &mut Unicorn<'_, *mut UCWrapper<'a>>| {
            hook();
        };
        // TODO: Store hook id so we can remove it?
        let _hook = self
            .uc_mut()
            .add_insn_sys_hook(InsnSysX86::SYSCALL, start.into(), end.into(), trampoline)
            .map_err(cpu_error_from_uc_error)?;
        Ok(())
    }

    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, MemoryAction, u64, usize, i64) -> bool + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<'_, *mut UCWrapper<'a>>, mem_type, address, size, value| {
            hook(unsafe { &mut **uc.get_data_mut() }, mem_type_to_memory_action(mem_type), address, size, value)
        };
        // TODO: Store hook id to be able to remove hook later on
        let _hook = self
            .uc_mut()
            .add_mem_hook(HookType::MEM_UNMAPPED, start.into(), end.into(), trampoline)
            .map_err(cpu_error_from_uc_error)?;
        Ok(())
    }

    fn add_block_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, EVAddr, u32) + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<'_, *mut UCWrapper<'a>>, address: u64, instruction_count: u32| {
            hook(unsafe { &mut **uc.get_data_mut() }, address.into(), instruction_count)
        };
        // TODO: Store hook id so we can remove it later
        let _hook = self
            .uc_mut()
            .add_block_hook(start.into(), end.into(), trampoline)
            .map_err(cpu_error_from_uc_error)?;
        Ok(())
    }

    fn add_vtlb_hook(
        &mut self,
        mut hook: Box<dyn FnMut(&mut Self, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<'_, *mut UCWrapper<'a>>, address: u64, access_type| {
            hook(unsafe { &mut **uc.get_data_mut() }, address.into(), mem_type_to_memory_action(access_type))
                .map(transform_tlb_entry_to_uc_tlb_entry)
        };
        self.uc_mut()
            .add_tlb_hook(1, 0, trampoline)
            .map_err(cpu_error_from_uc_error)?;
        Ok(())
    }

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        mut hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    ) -> Result<(), CPUError> {
        let trampoline = move |uc: &mut Unicorn<'_, *mut UCWrapper<'a>>, address, size| {
            hook(unsafe { &mut **uc.get_data_mut() }, address, size)
        };
        let _hook = self
            .uc_mut()
            .add_code_hook(start.into(), end.into(), trampoline)
            .map_err(cpu_error_from_uc_error)?;
        Ok(())
    }
}

fn transform_tlb_entry_to_uc_tlb_entry(value: TLBEntry) -> TlbEntry {
    TlbEntry {
        paddr: value.physical_address.into(),
        perms: Permission::from_bits(value.permissions.bits()).unwrap(),
    }
}

fn mem_type_to_memory_action(memtype: MemType) -> MemoryAction {
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
