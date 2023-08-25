use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPUError;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::mmu::MemoryAction;
use crate::emulator::system::memory::TLBEntry;

type UnmappedMemHook<'a, Slf> = Box<dyn FnMut(&mut Slf, MemoryAction, u64, usize, i64) -> bool + 'a>;
type BlockHook<'a, Slf> = Box<dyn FnMut(&mut Slf, u64, u32) + 'a>;
type VtlbHook<'a, Slf> = Box<dyn FnMut(&mut Slf, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>;
type CodeHook<'a, Slf> = Box<dyn FnMut(&mut Slf, u64, u32) + 'a>;

pub trait Hooks<'a> {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        syscall: Box<dyn FnMut(&mut Self) + 'a>,
    ) -> Result<(), CPUError>;

    // TODO: This needs to be moved to the tlb since we want to hook the virtual address space
    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr, // FIXME These are actually physical addresses
        end: EVAddr,
        hook: UnmappedMemHook<'a, Self>,
    ) -> Result<(), CPUError>;

    fn add_block_hook(
        &mut self,
        hook: BlockHook<'a, Self>,
    ) -> Result<(), CPUError>;

    fn add_vtlb_hook(
        &mut self,
        hook: VtlbHook<'a, Self>,
    ) -> Result<(), CPUError>;

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: CodeHook<'a, Self>,
    ) -> Result<(), CPUError>;
}
