use std::ops::Not;

use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::inet6::tcp::TCPData;
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
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        // todo!();
        let tcp = fd.tcp.as_mut()?;
        tcp.tcp_system.borrow_mut().poll(tcp.id, interest).ok()
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
        let tcp = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        tcp.tcp_system.borrow_mut().close(tcp.id)?;
        Ok(())
    }

    fn bind(
        &mut self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        let tcp = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        Ok(tcp.tcp_system.borrow_mut().bind(tcp.id, sockaddr)?)
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        let tcp = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        Ok(tcp.tcp_system.borrow_mut().send(tcp.id, data)?)
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        let tcp = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        let (data, addr) = tcp.tcp_system.borrow_mut().recvfrom(tcp.id, len)?;
        Ok((data, addr))
        // Ok((
        //     fd.tcp.as_mut().unwrap().pending_data.pop_back().unwrap(),
        //     SocketAddress::IN6 {
        //         sin6_port: 5555,
        //         sin6_flowinfo: 0,
        //         sin6_addr: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        //         sin6_scope_id: 0,
        //     },
        // ))
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
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
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        let fd = fd.tcp.as_mut().ok_or(FileDescriptorError::NotASocket)?;
        let (new_id, sockaddr) = fd.tcp_system.borrow_mut().accept(fd.id)?;
        let mut new_fd = FileDescriptor::new(Box::new(TcpSocketOps::new()));
        new_fd.data.tcp = Some(TCPData::new(fd.tcp_system.clone(), new_id));

        Ok((new_fd, sockaddr.into()))
    }

    fn listen(
        &mut self,
        fd: &mut Self::D,
        backlog: usize,
    ) -> Result<(), FileDescriptorError> {
        let tcp = fd.tcp.as_mut().unwrap();
        Ok(tcp.tcp_system.borrow_mut().listen(tcp.id, backlog)?)
    }
}
