use std::cell::Ref;
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::file_descriptors::{FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::{FD, FileDescriptorData};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::filesystem::{FSEntryId, Stat};
use crate::emulator::system::System;

#[derive(Debug, Clone)]
pub struct FileOps<Endianness: ByteOrder> {
    system: Rc<LinuxSystem<Endianness>>,
}

//TODO build a trait
impl<Endianness: ByteOrder> FileDescriptorOps<FileDescriptorData> for FileOps<Endianness> {
    fn poll(&self, fd: &mut FileDescriptorData) -> Option<FileOperationsReady> {
        None
    }

    fn supports_poll(&self, fd: &mut FileDescriptorData) -> bool {
        false
    }

    fn stat(&self, fd: &mut FileDescriptorData) -> Result<Ref<Stat>, GenericError> {
        let fd = fd.file.as_mut().ok_or(GenericError::new(GenericErrorKind::Unknown, "not a fs backed file"))?;
        Ref::filter_map(self.system.filesystem.borrow(), |fs| fs.stat(fd.fse_id).ok())
            .or(Err(GenericError::new(GenericErrorKind::Unknown, "Could not stat the file")))
    }

    fn read(
        &mut self,
        fd: &mut FileDescriptorData,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        let fd = fd.file.as_mut().ok_or(GenericError::new(GenericErrorKind::Unknown, "not a fs backed file"))?;
        let result = self.system.filesystem.borrow().read(fd.fse_id, fd.pos, count as usize)?;
        fd.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        fd: &mut FileDescriptorData,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        let fd = fd.file.as_mut().ok_or(GenericError::new(GenericErrorKind::Unknown, "not a fs backed file"))?;
        Ok(self.system.filesystem.borrow().read(fd.fse_id, offset as usize, length as usize)?)
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        let fd = fd.file.as_mut().ok_or(GenericError::new(GenericErrorKind::Unknown, "not a fs backed file"))?;
        let written = self.system.filesystem.borrow_mut().write(fd.fse_id, fd.pos, data)?;
        fd.pos += written;
        Ok(written)
    }
}

impl<Endianness: ByteOrder> FileOps<Endianness> {
    pub fn new(
        system: Rc<LinuxSystem<Endianness>>,
    ) -> FileOps<Endianness> {
        FileOps {
            system,
        }
    }
}
