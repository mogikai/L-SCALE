use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::platform::linux::kernel::socket::Optval;
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
        interest: FileOperationsReady,
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

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn sendto(
        &self,
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
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<Optval, FileDescriptorError> {
        todo!()
    }

    fn accept(
        &self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        todo!()
    }
}
