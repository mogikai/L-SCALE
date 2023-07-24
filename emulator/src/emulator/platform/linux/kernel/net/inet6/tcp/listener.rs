use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;

#[derive(Debug, Default)]
pub struct TcpListenerOps {}

impl TcpListenerOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps<FileDescriptorData> for TcpListenerOps {
    fn poll(&self, fd: &mut FileDescriptorData) -> Option<FileOperationsReady> {
        // TODO: Delegate to event system whether or not this socket is ready?
        warn!("Poll currently only has dummy implementation for tcp listener");
        Some(FileOperationsReady::Read)
    }

    fn supports_poll(&self, fd: &mut FileDescriptorData) -> bool {
        true
    }

    fn accept(&mut self, fd: &mut FileDescriptorData) -> Result<(FileDescriptor<FileDescriptorData>, Sockaddr), FileDescriptorError> {
        todo!()
    }
}
