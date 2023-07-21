use bitflags::Flags;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::{FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::filedescriptor::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;

#[derive(Debug, Default)]
pub struct TcpSock {}

impl TcpSock {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for TcpSock {
    fn poll(&self, fd: &mut FileDescriptorData) -> Option<FileOperationsReady> {
        todo!();
        Some(FileOperationsReady::Read)
    }

    fn supports_poll(&self, fd: &mut FileDescriptorData) -> bool {
        true
    }

    fn bind(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        Ok(())
    }

    fn setsockopt(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(), GenericError> {
        Ok(())
    }
}
