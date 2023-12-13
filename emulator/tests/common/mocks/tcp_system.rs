use mockall::mock;

use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
use fuzzyemu::emulator::abstractions::network::tcp_system::TCPSystem;
use fuzzyemu::emulator::abstractions::network::{
    NetworkError,
    SocketAddress,
    SocketId,
};
use fuzzyemu::emulator::abstractions::restore::Restorable;

mock! {
    #[derive(Debug)]
    pub TCPSystem {}
    impl TCPSystem for TCPSystem {
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

    fn poll(
        &self,
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
    }
    impl Restorable for TCPSystem {
        fn take_snapshot(&mut self) -> Result<(), GenericError>;
        fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError>;
        fn restore_snapshot(&mut self) -> Result<(), GenericError>;
    }
}
