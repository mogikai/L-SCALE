use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::emulator::abstractions::tcp_system::TCPSystem;

pub struct TCP {}

impl TCP {
    pub fn new() -> Self {
        TCP {}
    }
}

impl Restorable for TCP {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}

impl TCPSystem for TCP {}
