use crate::emulator::abstractions::errors::GenericError;

pub trait Restorable {
    fn snapshot(&mut self) -> Result<(), GenericError>;
    fn restore_and_discard(&mut self) -> Result<(), GenericError>;
    fn restore(&mut self) -> Result<(), GenericError>;
}
