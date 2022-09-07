use crate::emulator::abstractions::errors::GenericError;

pub trait Snapshot {
    fn snapshot(&mut self) -> Result<(), GenericError>;
}