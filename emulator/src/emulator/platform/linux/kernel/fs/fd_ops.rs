use std::cell::Ref;
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorError, FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::net::Sockaddr;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::filesystem::Stat;

#[derive(Debug, Clone)]
pub struct FileOps<Endianness: ByteOrder> {
    system: Rc<LinuxSystem<Endianness>>,
}

//TODO build a trait
impl<Endianness: ByteOrder> FileDescriptorOps<FileDescriptorData> for FileOps<Endianness> {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        None
    }

    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        false
    }

    fn stat(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Result<Ref<Stat>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        Ref::filter_map(self.system.filesystem.borrow(), |fs| fs.stat(fd.fse_id).ok())
            .or(Err(FileDescriptorError::OperationFailed))
    }

    fn read(
        &mut self,
        fd: &mut FileDescriptorData,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        let result = self
            .system
            .filesystem
            .borrow()
            .read(fd.fse_id, fd.pos, count as usize)?;
        fd.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        fd: &mut FileDescriptorData,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        Ok(self
            .system
            .filesystem
            .borrow()
            .read(fd.fse_id, offset as usize, length as usize)?)
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        let fd = fd.file.as_mut().ok_or(FileDescriptorError::NotSupported)?;
        let written = self
            .system
            .filesystem
            .borrow_mut()
            .write(fd.fse_id, fd.pos, data)?;
        fd.pos += written;
        Ok(written)
    }

    fn sendto(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
        target: &Option<Sockaddr>,
    ) -> Result<usize, FileDescriptorError> {
        todo!()
    }

    fn recvfrom(
        &self,
        fd: &mut FileDescriptorData,
        len: usize,
    ) -> Result<(Vec<u8>, Sockaddr), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn connect(
        &mut self,
        fd: &mut FileDescriptorData,
        sockaddr: Sockaddr,
    ) -> Result<(), FileDescriptorError> {
        Err(FileDescriptorError::NotSupported)
    }

    fn accept(
        &mut self,
        fd: &mut FileDescriptorData,
    ) -> Result<(FileDescriptor<FileDescriptorData>, Sockaddr), FileDescriptorError> {
        Err(FileDescriptorError::NotASocket)
    }
}

impl<Endianness: ByteOrder> FileOps<Endianness> {
    pub fn new(system: Rc<LinuxSystem<Endianness>>) -> FileOps<Endianness> {
        FileOps { system }
    }
}
