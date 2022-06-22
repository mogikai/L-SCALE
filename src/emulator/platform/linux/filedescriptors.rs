use std::cmp::min;

use crate::emulator::abstractions::descriptors::{Handle, IOErr};
use crate::emulator::system::filesystem::FSEntry;

#[derive(Debug, Clone)]
pub struct OpenFile {
    f: FSEntry,
    pos: usize,
}

//TODO build a trait
impl Handle for OpenFile {
    fn stat(&self) -> Vec<u8> { self.f.stat() }

    fn read(&mut self, count: u64) -> Result<&[u8], IOErr> {
        match &self.f {
            FSEntry::FileEntry(file, _) => {
                let c = count.try_into().unwrap();
                let result = Ok(&file[self.pos..][..c]);
                self.pos += c;
                result
            }
            FSEntry::DirEntry(_, _) => Err(IOErr::EISDIR),
        }
    }

    fn pread(&self, offset: u64, length: u64) -> Result<&[u8], IOErr> {
        match &self.f {
            FSEntry::FileEntry(file, _) => {
                let s = offset.try_into().unwrap();
                let l = min(file.size() - s, length.try_into().unwrap());
                Ok(&file[s..][..l])
            }
            FSEntry::DirEntry(_, _) => Err(IOErr::EISDIR),
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), IOErr> {
        todo!()
    }
}

impl OpenFile {
    pub fn new(file: FSEntry) -> OpenFile {
        OpenFile { f: file, pos: 0 }
    }
    pub fn stat(&self) -> Vec<u8> {
        self.f.stat()
    }
}