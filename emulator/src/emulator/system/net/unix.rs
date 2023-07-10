use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use byteorder::ByteOrder;

use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::filesystem::FSError;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;

pub struct UnixSocketData {}
#[derive(Default)]
pub struct UnixSocket {}

impl UnixSocket {
    pub fn new() -> Self { Self {} }
}

impl Handle for UnixSocket {
    fn connect(&mut self, fd: &mut FileDescriptor) -> Result<(), GenericError> {
        Err(GenericError::new(GenericErrorKind::FSError(FSError::NotFound), ""))
    }
}
