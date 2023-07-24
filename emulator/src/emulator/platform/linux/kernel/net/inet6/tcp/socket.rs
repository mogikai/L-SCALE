use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug, Default)]
pub struct TcpSocketOps {}

impl TcpSocketOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps<FileDescriptorData> for TcpSocketOps {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        // todo!();
        Some(FileOperationsReady::Read)
    }

    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        true
    }

    fn bind(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn setsockopt(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn accept(&mut self, fd: &mut FileDescriptorData) -> Result<(FileDescriptor<FileDescriptorData>, Sockaddr), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }
}
