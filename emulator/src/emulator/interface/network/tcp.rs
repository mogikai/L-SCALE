use crate::emulator::abstractions::file_descriptors::FileOperationsReady;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::{
    NetworkError,
    SocketAddress,
    SocketId,
};
use std::collections::VecDeque;

/// Extends upon the `TCPSystem` trait to additionally provide control methods to control the available tcp streams
/// or send or receive data from these streams.
pub trait TCPSystemControl: TCPSystem {
    /// An iterator over tuples of all known socket_ids and their associated sockets
    fn sockets(&self) -> impl Iterator<Item = (&SocketId, &impl TCPSocket)>;

    /// A mutable iterator over tuples of all known socket_ids and their associated sockets.
    fn sockets_mut(&mut self) -> impl Iterator<Item = (&SocketId, &mut impl TCPSocket)>;

    /// Register a new incoming connection on the listener socket identified by `listener`
    fn new_connection(
        &mut self,
        listener: &SocketId,
        remote_address: SocketAddress,
    ) -> Result<SocketId, NetworkError>;

    fn socket(
        &self,
        id: SocketId,
    ) -> Option<&impl TCPSocket>;

    fn socket_mut(
        &mut self,
        id: SocketId,
    ) -> Option<&mut impl TCPSocket>;
}

pub trait TCPSocket {
    /// Returns the file operations this socket is currently interested in performing.
    fn interest(&self) -> FileOperationsReady;

    /// Returns the (local) address that the socket is bound to.
    fn bound_address(&self) -> Option<SocketAddress>;

    /// If this socket is a listener, the method returns true. Otherwise it will return false.
    fn is_listener(&self) -> bool;

    fn push_data(
        &mut self,
        data: &[u8],
    ) -> Result<(), NetworkError>;

    /// Takes all data that was sent to this socket by the process. This clears the outgoing buffer.
    fn take_output(&mut self) -> Result<VecDeque<Vec<u8>>, NetworkError>;

    /// Returns a reference to the output data, without removing it from the buffer.
    fn peek_output(&self) -> Result<&VecDeque<Vec<u8>>, NetworkError>;
}
