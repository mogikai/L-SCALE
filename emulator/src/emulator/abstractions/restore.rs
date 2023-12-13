use crate::emulator::abstractions::errors::GenericError;
use std::cell::RefCell;

/// The `Restorable` trait provides functionality for taking snapshots
/// of an object's state and restoring it to a previous state.
pub trait Restorable {
    /// Takes a snapshot of the current state of the object.
    /// The snapshot can later be restored using the `restore` function.
    ///
    /// # Errors
    ///
    /// Returns an error if taking a snapshot fails.
    fn take_snapshot(&mut self) -> Result<(), GenericError>;

    /// Restores the object's state to the previous snapshot.
    /// This can be used to go back in time and restore the
    /// object to an earlier state than the current snapshot.
    /// This will discard the current snapshot in favor of
    /// the previous snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if restoring the previous snapshot fails.
    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError>;

    /// Restores the object's state to the current snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if restoring the current snapshot fails.
    fn restore_snapshot(&mut self) -> Result<(), GenericError>;
}

impl<T> Restorable for RefCell<T>
where
    T: Restorable,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        self.borrow_mut().take_snapshot()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        self.borrow_mut()
            .discard_current_and_restore_previous_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        self.borrow_mut().restore_snapshot()
    }
}
