use std::cell::{Ref, RefCell};
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::{Filesystem, Stat};

#[derive(Debug, Clone)]
pub struct FileOps {}

//TODO build a trait
impl FileDescriptorOps for FileOps {
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

    fn stat<'a>(
        &self,
        fd: &'a mut Self::D,
    ) -> Result<Ref<'a, Stat>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        Ref::filter_map(fd.filesystem.borrow(), |fs| fs.stat(fd.fse_id).ok())
            .or(Err(FileDescriptorError::OperationFailed))
    }

    fn read(
        &mut self,
        fd: &mut Self::D,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        let result = fd
            .filesystem
            .borrow()
            .read(fd.fse_id, fd.pos, count as usize)?;
        fd.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        fd: &mut Self::D,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        Ok(fd
            .filesystem
            .borrow()
            .read(fd.fse_id, offset as usize, length as usize)?)
    }

    fn write(
        &mut self,
        fd: &mut Self::D,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        let written = fd.filesystem.borrow_mut().write(fd.fse_id, fd.pos, data)?;
        fd.pos += written;
        Ok(written)
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
        Err(FileDescriptorError::NotSupported)
    }

    fn connect(
        &mut self,
        fd: &mut Self::D,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn accept(
        &mut self,
        fd: &mut Self::D,
    ) -> Result<(FileDescriptor<Self::D>, Sockaddr), FileDescriptorError> {
        Err(FileDescriptorError::NotASocket)
    }
}

impl FileOps {
    pub fn new() -> FileOps {
        FileOps {}
    }
}
