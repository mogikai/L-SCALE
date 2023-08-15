use std::cell::Ref;
use std::fmt::Debug;

use bitflags::bitflags;
use derive_more::Display;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::mmu::MemoryError;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::{FSError, Stat};

/// The HandleOps trait contains all the functions necessary
/// for a handle (file descriptor in linux) to function.
/// Some functions may always return an error since they might
/// not be supported on the specific supplied handle.
///
/// The actual handle data is contained in the TODO struct
///
pub trait FileDescriptorOps<D: Default>: Debug {
    /// Can be called to query what operations can be called without blocking on the handle.
    fn poll(
        &self,
        fd: &mut D,
    ) -> Option<FileOperationsReady>;

    /// Returns true if the file descriptor supports polling.
    fn supports_poll(
        &self,
        fd: &mut D,
    ) -> bool;

    /// returns: The stat vector of the handle this method is called on
    fn stat<'a>(
        &'a self,
        fd: &'a mut D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        todo!()
    }

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(
        &mut self,
        fd: &mut D,
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
        fd: &mut D,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn bind(
        &mut self,
        fd: &mut D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn getsockname(
        &self,
        fd: &mut D,
    ) -> Result<&Sockaddr, FileDescriptorError> {
        todo!()
    }

    fn sendto(
        &mut self,
        fd: &mut D,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError>;

    fn recvmsg(
        &mut self,
        fd: &mut D,
        len: usize,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut D,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError>;

    fn connect(
        &mut self,
        fd: &mut D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError>;

    fn setsockopt(
        &mut self,
        fd: &mut D,
    ) -> Result<(), FileDescriptorError> {
        todo!()
    }

    fn accept(
        &mut self,
        fd: &mut D,
    ) -> Result<(FileDescriptor<D>, Sockaddr), FileDescriptorError>;
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
