use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug)]
pub(super) struct EpollOps {}

impl EpollOps {
    pub(super) fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps<FileDescriptorData> for EpollOps {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        true
    }

    fn sendto(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut FileDescriptorData,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError> {
        todo!()
    }

    fn accept(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(FileDescriptor<FileDescriptorData>, Sockaddr), FileDescriptorError> {
        todo!()
    }
}
