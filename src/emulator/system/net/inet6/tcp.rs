use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::net::Sockaddr;

pub struct TcpSock {}

impl TcpSock {
    pub fn new() -> Self { Self {} }
}

impl<Endianness: ByteOrder> Handle<Endianness> for TcpSock {
    fn bind(
        &mut self,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        Ok(())
    }

    fn setsockopt(&mut self) -> Result<(), GenericError> { Ok(()) }
}
