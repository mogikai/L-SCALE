use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileOperationsReady};
use std::fmt::Debug;

use crate::emulator::abstractions::network::{
    NetworkError,
    ShutdownMethod,
    SocketAddress,
    SocketId,
};
use mogi_system::components::restore::Restorable;

pub trait TCPSystem: Restorable + Debug {
    fn new_socket(&mut self) -> Result<SocketId, NetworkError>;

    fn bind(
        &mut self,
        id: SocketId,
        address: SocketAddress,
    ) -> Result<(), NetworkError>;

    fn close(
        &mut self,
        id: SocketId,
    ) -> Result<(), NetworkError>;

    fn accept(
        &mut self,
        id: SocketId,
    ) -> Result<(SocketId, SocketAddress), NetworkError>;

    /// Puts the socket identified by `id` into listen mode.
    fn listen(
        &mut self,
        id: SocketId,
        backlog: usize,
    ) -> Result<(), NetworkError>;

    fn shutdown(
        &mut self,
        id: SocketId,
        how: ShutdownMethod,
    ) -> Result<(), NetworkError>;

    fn poll(
        &mut self,
        id: SocketId,
        interest: FileOperationsReady,
    ) -> Result<FileOperationsReady, NetworkError>;

    fn send(
        &mut self,
        id: SocketId,
        data: &[u8],
    ) -> Result<usize, NetworkError>;

    fn recvfrom(
        &mut self,
        id: SocketId,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), NetworkError>;
    
    fn getsockname(&self, id: SocketId) -> Result<SocketAddress, NetworkError>;
}
