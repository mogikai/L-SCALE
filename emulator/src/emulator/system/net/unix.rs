use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use byteorder::ByteOrder;

use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::filesystem::FSError;
use crate::emulator::platform::linux::filedescriptor::{FileDescriptor, FileDescriptorData};

pub struct UnixSocketData {}
#[derive(Default)]
pub struct UnixSocketOps {}

impl UnixSocketOps {
    pub fn new() -> Self { Self {} }
}

impl HandleOps for UnixSocketOps {
    fn connect(&mut self, fd: &mut FileDescriptorData) -> Result<(), GenericError> {
        Err(GenericError::new(GenericErrorKind::FSError(FSError::NotFound), ""))
    }
}
