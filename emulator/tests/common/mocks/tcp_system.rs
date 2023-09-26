use mockall::mock;

use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::emulator::abstractions::tcp_system::TCPSystem;

mock! {
    #[derive(Debug)]
    pub TCPSystem {}
    impl TCPSystem for TCPSystem {}
    impl Restorable for TCPSystem {
        fn snapshot(&mut self) -> Result<(), GenericError>;
        fn restore_and_discard(&mut self) -> Result<(), GenericError>;
        fn restore(&mut self) -> Result<(), GenericError>;
    }
}
