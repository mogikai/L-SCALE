use mockall::mock;

use mogi_emulator::emulator::abstractions::errors::GenericError;
use mogi_emulator::emulator::abstractions::file_descriptors::FileOperationsReady;
use mogi_emulator::emulator::abstractions::network::tcp_system::TCPSystem;
use mogi_emulator::emulator::abstractions::network::{
    NetworkError,
    ShutdownMethod,
    SocketAddress,
    SocketId,
};
use mogi_emulator::emulator::abstractions::restore::Restorable;

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
    fn shutdown(
        &mut self,
        id: SocketId,
        how: ShutdownMethod,
    ) -> Result<(), NetworkError>;
    }
    impl Restorable for TCPSystem {
        fn take_snapshot(&mut self) -> Result<(), GenericError>;
        fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError>;
        fn restore_snapshot(&mut self) -> Result<(), GenericError>;
    }
}
