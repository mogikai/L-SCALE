use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::Clock;

impl Restorable for Clock {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(self.system_time);
        Ok(())
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.system_time = self
            .snapshots
            .pop()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restor"))?;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.system_time = *self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restor"))?;
        Ok(())
    }
}
