use std::cell::Ref;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::system::filesystem::Stat;
use crate::emulator::system::net::Sockaddr;

pub trait Handle<Endianness: ByteOrder> {
    /// returns: The stat vector of the handle this method is called on
    fn stat(
        &self
    ) -> Result<Ref<Stat<Endianness>>, GenericError> {
        todo!()
    }

    /// # Arguments
    /// * `count`: The amount of bytes to read from the current position of the descriptor
    ///
    /// returns: A reference to the data that is accessed by the read call
    fn read(
        &mut self,
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
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn write(
        &mut self,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn bind(
        &mut self,
        sockaddr: Sockaddr,
    ) -> Result<(), GenericError> {
        todo!()
    }

    fn getsockname(
        &self,
    ) -> Result<&Sockaddr, GenericError> {
        todo!()
    }

    fn sendto(
        &mut self,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }

    fn recvmsg(
        &mut self,
        len: usize,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }
}