use std::ops::Not;

use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::tcp_system::TCPSystem;
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

impl FileDescriptorOps for TcpSocketOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
    ) -> Option<FileOperationsReady> {
        // todo!();
        let tcp = fd.tcp.as_mut()?;
        tcp.pending_data
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

    fn bind(
        &mut self,
        fd: &mut Self::D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError> {
        let tcp = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        tcp.out_data.push_front(data.to_vec());
        Ok(data.len())
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError> {
        Ok((
            fd.tcp.as_mut().unwrap().pending_data.pop_back().unwrap(),
            Sockaddr::IN6 {
                sin6_port: 5555,
                sin6_flowinfo: 0,
                sin6_addr: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
                sin6_scope_id: 0,
            },
        ))
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn setsockopt(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, Sockaddr), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }
}
