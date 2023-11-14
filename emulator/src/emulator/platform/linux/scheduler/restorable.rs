use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::{
    GenericError,
    GenericErrorKind,
};
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::scheduler::LinuxX86Scheduler;
use byteorder::ByteOrder;
use std::collections::BTreeMap;
use std::mem::take;

impl<'a, CpuT, TcpT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + MMU,
    TcpT: TCPSystem,
    Endianness: 'static + ByteOrder,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.system.snapshot()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.system.restore()
    }
}
