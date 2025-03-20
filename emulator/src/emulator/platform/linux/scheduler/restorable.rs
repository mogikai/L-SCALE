use byteorder::ByteOrder;
use mogi_system::components::mmu::MMU;

use mogi_system::components::cpu::CPU;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::errors::generic::GenericError;

use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use mogi_system::components::restore::Restorable;

impl<'a, CpuT, MmuT, TcpT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + Restorable,
    MmuT: MMU,
    TcpT: TCPSystem,
    Endianness: 'static + ByteOrder,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        Ok(())
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        Ok(())
    }
}
