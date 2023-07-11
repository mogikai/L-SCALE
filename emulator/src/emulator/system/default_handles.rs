use std::cell::{Ref, RefCell};
use std::cmp::min;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::HandleOps;
use crate::emulator::system::filesystem::Stat;
use crate::emulator::platform::linux::filedescriptor::{FileDescriptor, FileDescriptorData};

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

impl HandleOps for WriteOnly {
    fn stat(&self, fd: &mut FileDescriptorData) -> Result<Ref<Stat>, GenericError> { Ok(self.stat.borrow()) }

    fn read(
        &mut self,
        fd: &mut FileDescriptorData,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn pread(
        &self,
        fd: &mut FileDescriptorData,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        print!("{:}", String::from_utf8_lossy(data));
        Ok(data.len())
    }
}

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

impl HandleOps for ReadOnly {
    fn stat(&self, fd: &mut FileDescriptorData) -> Result<Ref<Stat>, GenericError> { Ok(self.stat.borrow()) }

    fn read(
        &mut self,
        fd: &mut FileDescriptorData,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        let upper = min(self.pos + count as usize, self.data.len());
        let result = self.data[self.pos..upper].to_vec();
        self.pos += result.len();
        Ok(result)
    }

    fn pread(
        &self,
        fd: &mut FileDescriptorData,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

    fn write(
        &mut self,
        fd: &mut FileDescriptorData,
        data: &[u8],
    ) -> Result<usize, GenericError> {
        todo!()
    }
}
