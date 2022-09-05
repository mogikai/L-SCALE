use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use crate::emulator::abstractions::handles::{FSError, Handle};
use crate::emulator::system::filesystem::{Filesystem, FSEntryId};

#[derive(Debug, Clone)]
pub struct OpenFile {
    filesystem: Rc<RefCell<Filesystem>>,
    fse_id: FSEntryId,
    pos: usize,
}

//TODO build a trait
impl Handle for OpenFile {
    fn stat(&self) -> Vec<u8> { self.filesystem.borrow().stat(self.fse_id).unwrap().into() }

    fn read(&mut self, count: u64) -> Result<Vec<u8>, FSError> {
        let result = self.filesystem.borrow().read(self.fse_id, self.pos, count as usize)?;
        self.pos += result.len();
        Ok(result)
    }

    fn pread(&self, offset: u64, length: u64) -> Result<Vec<u8>, FSError> {
        self.filesystem.borrow().read(self.fse_id, offset as usize, length as usize)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, FSError> {
        let written = self.filesystem.borrow_mut().write(self.fse_id, self.pos, data)?;
        self.pos += written;
        Ok(written)
    }
}

impl OpenFile {
    pub fn new(fs: Rc<RefCell<Filesystem>>, file: FSEntryId) -> OpenFile {
        OpenFile { filesystem: fs, pos: 0, fse_id: file }
    }
    pub fn stat(&self) -> Vec<u8> {
        self.filesystem.borrow().stat(self.fse_id).unwrap().into()
    }
}