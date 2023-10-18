use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug)]
pub(super) struct EpollOps {}

impl EpollOps {
    pub(super) fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for EpollOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        true
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        todo!()
    }
}
