use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::mmu::MemoryAction;
use crate::emulator::system::memory::TLBEntry;

type UnmappedMemHook<Slf> = Box<dyn FnMut(&mut Slf, MemoryAction, u64, usize, i64) -> bool + 'static>;
type BlockHook<Slf> = Box<dyn FnMut(&mut Slf, EVAddr, u32) + 'static>;
type VtlbHook<Slf> = Box<dyn FnMut(&mut Slf, EVAddr, MemoryAction) -> Option<TLBEntry> + 'static>;
type CodeHook<Slf> = Box<dyn FnMut(&mut Slf, u64, u32) + 'static>;

pub trait Hooks {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        syscall: Box<dyn FnMut() + 'static>,
    ) -> Result<(), CPUError>;

    // TODO: This needs to be moved to the tlb since we want to hook the virtual address space
    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr, // FIXME These are actually physical addresses
        end: EVAddr,
        hook: UnmappedMemHook<Self>,
    ) -> Result<(), CPUError>;

    fn add_block_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: BlockHook<Self>,
    ) -> Result<(), CPUError>;

    fn add_vtlb_hook(
        &mut self,
        hook: VtlbHook<Self>,
    ) -> Result<(), CPUError>;

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: CodeHook<Self>,
    ) -> Result<(), CPUError>;
}
