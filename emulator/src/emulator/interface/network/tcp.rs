use crate::emulator::abstractions::file_descriptors::FileOperationsReady;
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::{
    NetworkError,
    SocketAddress,
    SocketId,
};

/// Extends upon the `TCPSystem` trait to additionally provide control methods to control the available tcp streams
/// or send or receive data from these streams.
pub trait TCPSystemControl: TCPSystem {
    fn sockets(&self) -> impl Iterator<Item = (&SocketId, &impl TCPSocket)>;
    fn sockets_mut(&mut self) -> impl Iterator<Item = (&SocketId, &mut impl TCPSocket)>;
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
}
