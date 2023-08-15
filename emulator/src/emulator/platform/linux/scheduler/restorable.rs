use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::scheduler::{LinuxX86Scheduler, SchedulerContext};
use byteorder::ByteOrder;

impl<'a, CpuT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + Hooks<'a> + MMU,
    Endianness: 'static + ByteOrder,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.system.snapshot()?;
        self.context.borrow_mut().snapshot()?;
        self.cpu.borrow_mut().snapshot()?;
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}

impl<Endianness: ByteOrder> Restorable for SchedulerContext<Endianness> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
