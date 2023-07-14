use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::filedescriptor::FileDescriptorData;
use crate::emulator::system::filesystem::FSError;

#[derive(Debug)]
pub struct UnixSocketData {}

#[derive(Default, Debug)]
pub struct UnixSocketOps {}

impl UnixSocketOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for UnixSocketOps {
    fn connect(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(), GenericError> {
        Err(GenericError::new(GenericErrorKind::FSError(FSError::NotFound), ""))
    }
}
