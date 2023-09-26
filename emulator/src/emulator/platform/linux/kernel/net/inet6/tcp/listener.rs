use std::ops::Not;

use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::socket::TcpSocketOps;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug, Default)]
pub struct TcpListenerOps {}

impl TcpListenerOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for TcpListenerOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
    ) -> Option<FileOperationsReady> {
        // TODO: Delegate to event system whether or not this socket is ready?
        warn!("Poll currently only has dummy implementation for tcp listener");
        fd.tcp
            .as_mut()
            .unwrap()
            .pending_connections
            .is_empty()
            .not()
            .then_some(FileOperationsReady::Read)
            .or(Some(FileOperationsReady::empty()))
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
        Err(FileDescriptorError::NotSupported)
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
        let fd = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        let mut new_fd = FileDescriptor::new(Box::new(TcpSocketOps::new()));
        new_fd.data.tcp = Some(TCPData::new(fd.tcp_system.clone()));

        Ok((new_fd, fd.pending_connections.pop().unwrap()))
    }
}
