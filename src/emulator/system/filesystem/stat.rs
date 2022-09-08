use std::fs::Metadata;
use std::marker::PhantomData;
use std::mem;
use std::mem::size_of;
use std::os::linux::fs::MetadataExt;

use byteorder::{ByteOrder, LittleEndian};

use crate::emulator::system::memory::{MemoryError, WriteToMemory};

#[derive(Clone, Debug, Default)]
pub struct Stat<Endianness: ByteOrder> {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    _pad0: u32,
    pub st_rdev: u64,
    pub st_size: u64,
    pub st_blksize: u64,
    pub st_blocks: u64,
    /* Number 512-byte blocks allocated. */
    pub st_atime: u64,
    pub st_atime_nsec: u64,
    pub st_mtime: u64,
    pub st_mtime_nsec: u64,
    pub st_ctime: u64,
    pub st_ctime_nsec: u64,

    _unused1: u64,
    _unused2: u64,
    _unused3: u64,
    _phantom: PhantomData<Endianness>
}

impl<Endianness: ByteOrder> From<Metadata>
for Stat<Endianness> {
    fn from(m: Metadata) -> Self {
        Self {
            st_dev: m.st_dev(),
            st_ino: m.st_ino(),
            st_nlink: m.st_nlink(),
            st_mode: m.st_mode(),
            st_uid: m.st_uid(),
            st_gid: m.st_gid(),
            _pad0: 0,
            st_rdev: m.st_rdev(),
            st_size: m.st_size(),
            st_blksize: m.st_blksize(),
            st_blocks: m.st_blocks(),
            st_atime: m.st_atime() as u64,
            st_atime_nsec: m.st_atime_nsec() as u64,
            st_mtime: m.st_mtime() as u64,
            st_mtime_nsec: m.st_mtime_nsec() as u64,
            st_ctime: m.st_ctime() as u64,
            st_ctime_nsec: m.st_ctime_nsec() as u64,
            _unused1: 0,
            _unused2: 0,
            _unused3: 0,
            _phantom: Default::default()
        }
    }
}

impl<Endianness: ByteOrder> WriteToMemory<Endianness>
for &Stat<Endianness> {
    fn write_to_memory(&self, data: &mut [u8]) -> Result<(), MemoryError> {
        Endianness::write_u64(&mut data[0..8], self.st_dev);
        Endianness::write_u64(&mut data[8..16], self.st_ino);
        Endianness::write_u64(&mut data[16..24], self.st_nlink);
        Endianness::write_u32(&mut data[24..28], self.st_mode);
        Endianness::write_u32(&mut data[28..32], self.st_uid);
        Endianness::write_u32(&mut data[32..36], self.st_gid);
//		Endianness::write_u32(&mut data[36..40], self._pad0);
        Endianness::write_u64(&mut data[40..48], self.st_rdev);
        Endianness::write_u64(&mut data[48..56], self.st_size);
        Endianness::write_u64(&mut data[56..64], self.st_blksize);
        Endianness::write_u64(&mut data[64..72], self.st_blocks);
        Endianness::write_u64(&mut data[72..80], self.st_atime);
        Endianness::write_u64(&mut data[80..88], self.st_atime_nsec);
        Endianness::write_u64(&mut data[88..96], self.st_mtime);
        Endianness::write_u64(&mut data[96..104], self.st_mtime_nsec);
        Endianness::write_u64(&mut data[104..112], self.st_ctime);
        Endianness::write_u64(&mut data[112..120], self.st_ctime_nsec);
        Ok(())
    }

    fn required_size(&self) -> usize {
        mem::size_of::<u64>() * 20
    }
}
