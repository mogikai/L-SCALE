use std::cell::Ref;
use std::fmt::Debug;

use bitflags::bitflags;
use derive_more::Display;
use mogi_system::errors::filesystem::FSError;
use mogi_utilities::error::memory::MemoryError;

use crate::emulator::abstractions::network::{
    ShutdownMethod,
    SocketAddress,
};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::kernel::net::sockopt::Sockopt;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::platform::linux::kernel::socket::Optval;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::Stat;
use mogi_system::errors::generic::GenericError;

/// The HandleOps trait contains all the functions necessary
/// for a handle (file descriptor in linux) to function.
/// Some functions may always return an error since they might
/// not be supported on the specific supplied handle.
///
/// The actual handle data is contained in the TODO struct
///
pub trait FileDescriptorOps {
    type D: Default + Clone;
    /// Can be called to query what operations can be called without blocking on the handle.
    /// The interest parameter notifies the subsystems what the caller was interested in.
    /// This parameter should not influence the output and is there to let fuzzer components
    /// know what the last interest for a specific file descriptor was.
    /// That way the fuzzer can provide these demands in the future.
    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady>;

    /// Returns true if the file descriptor supports polling.
    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool;

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError>;

    /// returns: The stat vector of the handle this method is called on
    fn stat<'a>(
        &'a self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        todo!()
    }

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(
        &self,
        fd: &mut Self::D,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    /// # Arguments
    /// * `offset`: The offset to read from
    /// * `length`: The length of the data to be read
    ///
    /// returns: A reference to the data that is accessed by the pread call
    fn pread(
        &self,
        fd: &mut Self::D,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn write(
        &self,
        fd: &mut Self::D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn bind(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockname<'a>(
        &self,
        fd: &'a mut Self::D,
    ) -> Result<&'a SocketAddress, FileDescriptorError> {
        todo!()
    }

    fn sendto(
        &self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError>;

    fn recvmsg(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError>;

    fn connect(
        &self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError>;

    fn setsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockopt(
        &self,
        fd: &mut Self::D,
        sockopt: Sockopt,
    ) -> Result<Optval, FileDescriptorError>;

    fn accept(
        &self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError>;

    fn listen(
        &self,
        fd: &mut Self::D,
        backlog: usize,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn shutdown(
        &self,
        fd: &mut Self::D,
        how: ShutdownMethod,
    ) -> Result<(), FileDescriptorError>;
}

#[derive(Debug)]
pub enum FileDescriptorError {
    MemoryError(MemoryError),
    FSError(FSError),
    NotSupported,
    NotASocket,
    OperationFailed,
    Unknown,
    FileNotFound,
    Closed,
}

impl From<FSError> for FileDescriptorError {
    fn from(value: FSError) -> Self {
        FileDescriptorError::FSError(value)
    }
}

impl From<MemoryError> for FileDescriptorError {
    fn from(value: MemoryError) -> Self {
        FileDescriptorError::MemoryError(value)
    }
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Copy, Clone, Default)]
    pub struct FileOperationsReady: u64 {
        const Read              = 0x0001;
        const Priority          = 0x0002;
        const Write             = 0x0004;
        const Error             = 0x0008;
        const HangUp            = 0x0010;
        const Invalid           = 0x0020;
        const ReadNormal        = 0x0040;
        const ReadOutOfBand     = 0x0080;
        const WriteNormal       = 0x0100;
        const WriteOutOfBand    = 0x0200;
        const ReadHangUp        = 0x2000;
    }
}
