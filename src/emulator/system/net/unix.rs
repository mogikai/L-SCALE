use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use byteorder::ByteOrder;

use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::filesystem::FSError;

pub struct UnixSocket {}

impl UnixSocket {
    pub fn new() -> Self { Self {} }
}

impl Handle for UnixSocket {
    fn connect(&mut self) -> Result<(), GenericError> {
        Err(GenericError::new(GenericErrorKind::FSError(FSError::NotFound), ""))
    }
}
