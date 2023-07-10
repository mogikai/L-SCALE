use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use byteorder::ByteOrder;

use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::filesystem::FSError;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;

pub struct UnixSocketData {}
#[derive(Default)]
pub struct UnixSocket {}

impl UnixSocket {
    pub fn new() -> Self { Self {} }
}

impl HandleOps for UnixSocket {
    fn connect(&mut self, fd: &mut FileDescriptor) -> Result<(), GenericError> {
        Err(GenericError::new(GenericErrorKind::FSError(FSError::NotFound), ""))
    }
}
