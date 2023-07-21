use std::fs::Metadata;
use std::os::linux::fs::MetadataExt;

use fuzzyemu_derive::{Encode, RequiredMem};

use crate::emulator::abstractions::mmu::encoding::{Encode, RequiredMem};

#[derive(Clone, Debug, Default, Encode, RequiredMem)]
pub struct Stat {
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
}

impl From<Metadata> for Stat {
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
        }
    }
}
