use std::cell::Ref;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::system::filesystem::Stat;
use crate::emulator::system::net::Sockaddr;

use crate::emulator::platform::linux::filedescriptor::FileDescriptor;

pub trait Handle {
    fn read_ready(&self, fd: &mut FileDescriptor) -> bool { todo!() }
    fn write_ready(&self, fd: &mut FileDescriptor) -> bool { todo!() }

    /// returns: The stat vector of the handle this method is called on
    fn stat(&self, fd: &mut FileDescriptor) -> Result<Ref<Stat>, GenericError> { todo!() }

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(
        &mut self,
        fd: &mut FileDescriptor,
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
        fd: &mut FileDescriptor,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptor,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn bind(
        &mut self,
        fd: &mut FileDescriptor,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        todo!()
    }

    fn getsockname(&self, fd: &mut FileDescriptor) -> Result<&Sockaddr, GenericError> { todo!() }

    fn sendto(
        &mut self,
        fd: &mut FileDescriptor,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn recvmsg(
        &mut self,
        fd: &mut FileDescriptor,
        len: usize,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn connect(&mut self, fd: &mut FileDescriptor) -> Result<(), GenericError> { todo!() }

    fn setsockopt(&mut self, fd: &mut FileDescriptor) -> Result<(), GenericError> { todo!() }
}
