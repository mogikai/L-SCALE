use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug, Clone)]
pub struct SignalFD {
    mask: u64,
}

//TODO build a trait

impl SignalFD {
    pub fn new(mask: u64) -> Self {
        SignalFD { mask }
    }
}

#[derive(Debug, Default)]
pub struct SignalFDOps {}

impl SignalFDOps {
    pub fn new() -> Self {
        SignalFDOps {}
    }
}

impl FileDescriptorOps for SignalFDOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
    ) -> Option<FileOperationsReady> {
        // TODO: Implement poll operation instead of never signaling
        Some(FileOperationsReady::empty())
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        true
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
        todo!()
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, Sockaddr), FileDescriptorError> {
        todo!()
    }
}
