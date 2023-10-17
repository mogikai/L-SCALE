use mockall::mock;

use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::network::tcp_system::TCPSystem;
use fuzzyemu::emulator::abstractions::network::{NetworkError, SocketAddress, SocketId};
use fuzzyemu::emulator::abstractions::restore::Restorable;

mock! {
    #[derive(Debug)]
    pub TCPSystem {}
    impl TCPSystem for TCPSystem {
        fn new_socket(&mut self) -> Result<SocketId, NetworkError>;
        fn accept(&mut self, key: SocketId) -> Result<(SocketId, SocketAddress), NetworkError>;
        fn listen(&mut self, key: SocketId, backlog: usize) -> Result<(), NetworkError>;
    }
    impl Restorable for TCPSystem {
        fn snapshot(&mut self) -> Result<(), GenericError>;
        fn restore_and_discard(&mut self) -> Result<(), GenericError>;
        fn restore(&mut self) -> Result<(), GenericError>;
    }
}
