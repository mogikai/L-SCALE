use std::cell::{Ref, RefCell};
use std::cmp::min;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::Stat;

#[derive(Debug)]
pub struct WriteOnly {
    stat: RefCell<Stat>,
}

impl WriteOnly {
    pub(crate) fn new() -> Self {
        Self {
            stat: Default::default(),
        }
    }
}

// TODO: This probably should be handled via callbacks
impl FileDescriptorOps for WriteOnly {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        None
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        false
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn stat<'a>(
        &'a self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        Ok(self.stat.borrow())
    }

    fn read(
        &mut self,
        fd: &mut Self::D,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn pread(
        &self,
        fd: &mut Self::D,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        print!("{:}", String::from_utf8_lossy(data));
        Ok(data.len())
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }
}

#[derive(Debug)]
pub struct ReadOnly {
    data: Vec<u8>,
    pos: usize,
    stat: RefCell<Stat>,
}

impl ReadOnly {
    pub(crate) fn new() -> Self {
        Self {
            pos: 0,
            data: vec![],
            stat: Default::default(),
        }
    }

    pub fn from(data: Vec<u8>) -> Self {
        Self {
            pos: 0,
            data,
            stat: Default::default(),
        }
    }
}

impl FileDescriptorOps for ReadOnly {
    type D = FileDescriptorData;

    fn poll(
        &self,
        fd: &mut Self::D,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        None
    }

    fn supports_poll(
        &self,
        fd: &mut Self::D,
    ) -> bool {
        false
    }

    fn close(
        &self,
        fd: &mut Self::D,
    ) -> Result<(), FileDescriptorError> {
        Ok(())
    }

    fn stat<'a>(
        &'a self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        Ok(self.stat.borrow())
    }

    fn read(
        &mut self,
        fd: &mut Self::D,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        let upper = min(self.pos + count as usize, self.data.len());
        let result = self.data[self.pos..upper].to_vec();
        self.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        fd: &mut Self::D,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn sendto(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut Self::D,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        // TODO FIXME
        error!("UDP connect currently unsupported / not implemented");
        Err(FileDescriptorError::NotSupported)
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, SocketAddress), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }
}
