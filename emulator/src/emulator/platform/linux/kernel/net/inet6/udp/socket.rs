use std::fmt::Debug;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug)]
pub struct UdpSocketOps {}

impl UdpSocketOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for UdpSocketOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        todo!()
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, Sockaddr), FileDescriptorError> {
        todo!()
    }
}
