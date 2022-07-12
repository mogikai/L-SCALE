use crate::emulator::abstractions::handles::{Handle, IOErr};
use crate::emulator::system::filesystem::Stats;

pub struct WriteOnly {

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