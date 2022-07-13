use std::cmp::min;

use crate::emulator::abstractions::handles::{Handle, IOErr};
use crate::emulator::system::filesystem::Stats;

pub struct WriteOnly {}

impl WriteOnly {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Handle for WriteOnly {
    fn stat(&self) -> Vec<u8> {
        Stats::default().to_vec()
    }

    fn read(&mut self, count: u64) -> Result<&[u8], IOErr> {
        todo!()
    }

    fn pread(&self, offset: u64, length: u64) -> Result<&[u8], IOErr> {
        todo!()
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, IOErr> {
        print!("{:}", String::from_utf8_lossy(data));
        Ok(data.len())
    }
}

pub struct ReadOnly {
    data: Vec<u8>,
    pos: usize,
}

impl ReadOnly {
    pub(crate) fn new() -> Self {
        Self { pos: 0, data: vec![b'a', b'b', b'c', b'd', b'\n'] }
    }

    pub fn from(data: Vec<u8>) -> Self {
        Self { pos: 0, data }
    }
}

impl Handle for ReadOnly {
    fn stat(&self) -> Vec<u8> {
        Stats::default().to_vec()
    }

    fn read(&mut self, count: u64) -> Result<&[u8], IOErr> {
        let upper = min(self.pos + count as usize, self.data.len());
        Ok(&self.data[self.pos..upper])
    }

    fn pread(&self, offset: u64, length: u64) -> Result<&[u8], IOErr> {
        todo!()
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, IOErr> {
        todo!()
    }
}