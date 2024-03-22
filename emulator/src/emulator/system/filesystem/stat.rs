use mogi_derive::{
    Encode,
    RequiredMem,
};
use std::fs::Metadata;
use std::os::linux::fs::MetadataExt;

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

pub const S_IFMT: u32 = 0o0170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFIFO: u32 = 0o010000;
pub const S_ISUID: u32 = 0o004000;
pub const S_ISGID: u32 = 0o002000;
pub const S_ISVTX: u32 = 0o001000;

#[allow(non_snake_case)]
impl Stat {
    pub fn S_ISLNK(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFLNK)
    }
    pub fn S_ISREG(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFREG)
    }
    pub fn S_ISDIR(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFDIR)
    }
    pub fn S_ISCHR(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFCHR)
    }
    pub fn S_ISBLK(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFBLK)
    }
    pub fn S_ISFIFO(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFIFO)
    }
    pub fn S_ISSOCK(&self) -> bool {
        (((self.st_mode) & S_IFMT) == S_IFSOCK)
    }
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
