use std::cell::{Ref, RefCell};
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::filesystem::{Filesystem, FSEntryId, Stat};

#[derive(Debug, Clone)]
pub struct
FileHandle<Endianness: ByteOrder> {
    filesystem: Rc<RefCell<Filesystem<Endianness>>>,
    fse_id: FSEntryId,
    pos: usize,
}

//TODO build a trait
impl<Endianness: ByteOrder>
Handle<Endianness> for FileHandle<Endianness> {
    fn stat(
        &self
    ) -> Result<Ref<Stat<Endianness>>, GenericError> {
        Ref::filter_map(self.filesystem.borrow(), |fs| fs.stat(self.fse_id).ok())
            .or(Err(GenericError::new(GenericErrorKind::Unknown, "Could not stat the file")))
    }

    fn read(
        &mut self,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        let result = self.filesystem.borrow().read(self.fse_id, self.pos, count as usize)?;
        self.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        Ok(self.filesystem.borrow().read(self.fse_id, offset as usize, length as usize)?)
    }

    fn write(
        &mut self,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        let written = self.filesystem.borrow_mut().write(self.fse_id, self.pos, data)?;
        self.pos += written;
        Ok(written)
    }
}

impl<Endianness: ByteOrder>
FileHandle<Endianness> {
    pub fn new(
        fs: Rc<RefCell<Filesystem<Endianness>>>,
        file: FSEntryId,
    ) -> FileHandle<Endianness> {
        FileHandle { filesystem: fs, pos: 0, fse_id: file }
    }
}