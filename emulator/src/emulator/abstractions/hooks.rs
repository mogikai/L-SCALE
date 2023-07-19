use crate::emulator::abstractions::cpu::address::{EPAddr, EVAddr};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::mmu::MemoryAction;
use crate::emulator::system::memory::TLBEntry;

pub trait Hooks<'a> {
    fn add_syscall_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        syscall: Box<dyn FnMut(&mut Self) + 'a>,
    );

    fn add_unmapped_mem_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: Box<dyn FnMut(&mut Self, MemoryAction, u64, usize, i64) -> bool + 'a>,
    );

    fn add_block_hook(
        &mut self,
        hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    );

    fn add_vtlb_hook(
        &mut self,
        hook: Box<dyn FnMut(&mut Self, EVAddr, MemoryAction) -> Option<TLBEntry> + 'a>,
    ) -> Result<(), GenericError>;

    fn add_code_hook(
        &mut self,
        start: EVAddr,
        end: EVAddr,
        hook: Box<dyn FnMut(&mut Self, u64, u32) + 'a>,
    ) -> Result<(), GenericError>;
}
