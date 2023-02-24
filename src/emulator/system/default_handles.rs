use std::cell::{Ref, RefCell};
use std::cmp::min;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::system::filesystem::Stat;

pub struct
WriteOnly<Endianness: ByteOrder> {
    stat: RefCell<Stat<Endianness>>,
}

impl<Endianness: ByteOrder>
WriteOnly<Endianness> {
    pub(crate) fn new() -> Self {
        Self {
            stat: Default::default()
        }
    }
}

impl<Endianness: ByteOrder>
Handle<Endianness> for
WriteOnly<Endianness> {

    fn stat(
        &self
    ) -> Result<Ref<Stat<Endianness>>, GenericError> {
        Ok(self.stat.borrow())
    }

    fn read(
        &mut self,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        todo!()
    }

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
        print!("{:}", String::from_utf8_lossy(data));
        Ok(data.len())
    }
}

pub struct
ReadOnly<Endianness: ByteOrder> {
    data: Vec<u8>,
    pos: usize,
    stat: RefCell<Stat<Endianness>>,
}

impl<Endianness: ByteOrder>
ReadOnly<Endianness> {
    pub(crate) fn new() -> Self {
        Self { pos: 0, data: vec![], stat: Default::default() }
    }

    pub fn from(data: Vec<u8>) -> Self {
        Self { pos: 0, data, stat: Default::default() }
    }
}

impl<Endianness: ByteOrder>
Handle<Endianness> for
ReadOnly<Endianness> {

    fn stat(
        &self
    ) -> Result<Ref<Stat<Endianness>>, GenericError> {
        Ok(self.stat.borrow())
    }

    fn read(
        &mut self,
        count: u64,
    ) -> Result<Vec<u8>, GenericError> {
        let upper = min(self.pos + count as usize, self.data.len());
        let result = self.data[self.pos..upper].to_vec();
        self.pos += result.len();
        Ok(result)
    }

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
}