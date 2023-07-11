use std::cell::Ref;
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::platform::linux::filedescriptor::{FileDescriptor, FileDescriptorData};
use crate::emulator::system::filesystem::{FSEntryId, Stat};
use crate::emulator::system::System;

#[derive(Debug, Clone)]
pub struct FileData {
    fse_id: FSEntryId,
    pos: usize,
}

impl FileData {
    pub fn new(fse_id: FSEntryId) -> FileData {
        FileData { fse_id, pos: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct FileOps<Endianness: ByteOrder> {
    system: Rc<System<Endianness>>,
}

//TODO build a trait
impl<Endianness: ByteOrder> HandleOps for FileOps<Endianness> {
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
        system: Rc<System<Endianness>>,
    ) -> FileOps<Endianness> {
        FileOps {
            system,
        }
    }
}
