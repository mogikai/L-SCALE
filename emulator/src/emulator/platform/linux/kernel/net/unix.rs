use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::FSError;
use libc::sockaddr;
use std::ffi::CStr;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UnixSocketData {}

#[derive(Default, Debug)]
pub struct UnixSocketOps {}

impl UnixSocketOps {
    pub fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps for UnixSocketOps {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        todo!()
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError> {
        todo!()
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        // FIXME TODO
        error!("connect for unix socket not implemented");
        if let Sockaddr::UNIX { path } = sockaddr {
            debug!("Trying to connect to unix socket {:?}", CStr::from_bytes_until_nul(path.as_slice()));
        }
        Err(FileDescriptorError::FileNotFound)
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, Sockaddr), FileDescriptorError> {
        todo!()
    }
}
