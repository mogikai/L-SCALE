use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::filedescriptor::FileDescriptorData;
use crate::emulator::system::net::Sockaddr;

#[derive(Debug, Default)]
pub struct TcpSock {}

impl TcpSock {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for TcpSock {
    fn bind(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        Ok(())
    }

    fn setsockopt(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(), GenericError> {
        Ok(())
    }
}
