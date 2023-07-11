use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::net::Sockaddr;
use crate::emulator::platform::linux::filedescriptor::{FileDescriptor, FileDescriptorData};

pub struct TcpSock {}

impl TcpSock {
    pub fn new() -> Self { Self {} }
}

impl HandleOps for TcpSock {
    fn bind(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        Ok(())
    }

    fn setsockopt(&mut self, fd: &mut FileDescriptorData) -> Result<(), GenericError> { Ok(()) }
}
