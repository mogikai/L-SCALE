use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
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

impl FileDescriptorOps<FileDescriptorData> for SignalFDOps {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        // TODO: Implement poll operation instead of never signaling
        Some(FileOperationsReady::empty())
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
