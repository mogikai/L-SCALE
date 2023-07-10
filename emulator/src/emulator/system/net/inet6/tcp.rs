use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::net::Sockaddr;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;

pub struct TcpSock {}

impl TcpSock {
    pub fn new() -> Self { Self {} }
}

impl HandleOps for TcpSock {
    fn bind(
        &mut self,
        fd: &mut FileDescriptor,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        Ok(())
    }

    fn setsockopt(&mut self, fd: &mut FileDescriptor) -> Result<(), GenericError> { Ok(()) }
}
