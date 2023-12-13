use byteorder::ByteOrder;

use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::scheduler::LinuxX86Scheduler;

impl<'a, CpuT, TcpT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + MMU,
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
