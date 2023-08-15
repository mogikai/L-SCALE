use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::Clock;

impl Restorable for Clock {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(self.system_time);
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}
