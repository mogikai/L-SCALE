use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::Clock;
use mogi_system::errors::generic::{
    GenericError,
    GenericErrorKind,
};

impl Restorable for Clock {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.snapshots.push(self.system_time);
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "No previous snapshot available"))?;
        }
        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        self.system_time = *self
            .snapshots
            .last()
            .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restore"))?;
        Ok(())
    }
}
