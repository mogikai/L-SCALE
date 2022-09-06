use std::fs::Metadata;
use std::mem::size_of;
use std::os::linux::fs::MetadataExt;
use byteorder::{ByteOrder, LittleEndian};

#[derive(Clone, Debug, Default)]
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

impl From<Stat> for Vec<u8> {
    fn from(stat: Stat) -> Self {
        (&stat).into()
    }
}

impl From<&Stat> for Vec<u8> {
    fn from(stat: &Stat) -> Self {
        let mut v: Vec<u8> = vec![0; size_of::<Stat>()];
        LittleEndian::write_u64(&mut v[0..8], stat.st_dev);
        LittleEndian::write_u64(&mut v[8..16], stat.st_ino);
        LittleEndian::write_u64(&mut v[16..24], stat.st_nlink);
        LittleEndian::write_u32(&mut v[24..28], stat.st_mode);
        LittleEndian::write_u32(&mut v[28..32], stat.st_uid);
        LittleEndian::write_u32(&mut v[32..36], stat.st_gid);
//		LittleEndian::write_u32(&mut v[36..40], stat._pad0);
        LittleEndian::write_u64(&mut v[40..48], stat.st_rdev);
        LittleEndian::write_u64(&mut v[48..56], stat.st_size);
        LittleEndian::write_u64(&mut v[56..64], stat.st_blksize);
        LittleEndian::write_u64(&mut v[64..72], stat.st_blocks);
        LittleEndian::write_u64(&mut v[72..80], stat.st_atime);
        LittleEndian::write_u64(&mut v[80..88], stat.st_atime_nsec);
        LittleEndian::write_u64(&mut v[88..96], stat.st_mtime);
        LittleEndian::write_u64(&mut v[96..104], stat.st_mtime_nsec);
        LittleEndian::write_u64(&mut v[104..112], stat.st_ctime);
        LittleEndian::write_u64(&mut v[112..120], stat.st_ctime_nsec);
        v
    }
}