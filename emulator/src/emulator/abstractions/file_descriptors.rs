use std::cell::Ref;
use std::fmt::Debug;

use bitflags::bitflags;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::platform::linux::filedescriptor::FileDescriptorData;
use crate::emulator::system::filesystem::Stat;
use crate::emulator::system::net::Sockaddr;

/// The HandleOps trait contains all the functions necessary
/// for a handle (file descriptor in linux) to function.
/// Some functions may always return an error since they might
/// not be supported on the specific supplied handle.
///
/// The actual handle data is contained in the TODO struct
///
pub trait FileDescriptorOps: Debug {
    /// Can be called to query what operations can be called without blocking on the handle.
    fn poll(&self, fd: &mut FileDescriptorData) -> Option<FileOperationsReady>;

    /// Returns true if the file descriptor supports polling.
    fn supports_poll(&self, fd: &mut FileDescriptorData) -> bool;

    /// returns: The stat vector of the handle this method is called on
    fn stat(&self, fd: &mut FileDescriptorData) -> Result<Ref<Stat>, GenericError> { todo!() }

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(
        &mut self,
        fd: &mut FileDescriptorData,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    /// # Arguments
    /// * `offset`: The offset to read from
    /// * `length`: The length of the data to be read
    ///
    /// returns: A reference to the data that is accessed by the pread call
    fn pread(
        &self,
        fd: &mut FileDescriptorData,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn bind(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        todo!()
    }

    fn getsockname(&self, fd: &mut FileDescriptorData) -> Result<&Sockaddr, GenericError> { todo!() }

    fn sendto(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn recvmsg(
        &mut self,
        fd: &mut FileDescriptorData,
        len: usize,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn connect(&mut self, fd: &mut FileDescriptorData) -> Result<(), GenericError> { todo!() }

    fn setsockopt(&mut self, fd: &mut FileDescriptorData) -> Result<(), GenericError> { todo!() }
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