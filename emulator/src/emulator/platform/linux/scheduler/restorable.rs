use byteorder::ByteOrder;
use mogi_system::components::mmu::MMU;

use crate::emulator::abstractions::errors::GenericError;
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::components::cpu::CPU;

use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::scheduler::LinuxX86Scheduler;

impl<'a, CpuT, TcpT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + MMU + Restorable,
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
