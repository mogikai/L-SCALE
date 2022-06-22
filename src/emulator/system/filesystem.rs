use std::collections::HashMap;
use std::fs;
use std::fs::Metadata;
use std::io;
use std::mem;
use std::os::linux::fs::MetadataExt;
use std::path;
use std::path::{Path, PathBuf};

use byteorder::{ByteOrder, LittleEndian};

use crate::emulator::abstractions::descriptors::IOErr;

#[derive(Debug, Clone)]
pub struct File {
    data: Vec<u8>,
}

//TODO make dir a list of FSEntry or use a general tree
#[derive(Debug, Clone)]
pub struct Dir {}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    _pad0: u32,
    st_rdev: u64,
    st_size: u64,
    st_blksize: u64,
    st_blocks: u64,
    /* Number 512-byte blocks allocated. */
    st_atime: u64,
    st_atime_nsec: u64,
    st_mtime: u64,
    st_mtime_nsec: u64,
    st_ctime: u64,
    st_ctime_nsec: u64,

    _unused1: u64,
    _unused2: u64,
    _unused3: u64,
}

impl Stats {
    pub fn to_vec(&self) -> Vec<u8> {
        let mut v: Vec<u8> = vec![0; mem::size_of::<Stats>()];
        //TODO add support for diffrent endien
        LittleEndian::write_u64(&mut v[0..8], self.st_dev);
        LittleEndian::write_u64(&mut v[8..16], self.st_ino);
        LittleEndian::write_u64(&mut v[16..24], self.st_nlink);
        LittleEndian::write_u32(&mut v[24..28], self.st_mode);
        LittleEndian::write_u32(&mut v[28..32], self.st_uid);
        LittleEndian::write_u32(&mut v[32..36], self.st_gid);
//		LittleEndian::write_u32(&mut v[36..40], self._pad0);
        LittleEndian::write_u64(&mut v[40..48], self.st_rdev);
        LittleEndian::write_u64(&mut v[48..56], self.st_size);
        LittleEndian::write_u64(&mut v[56..64], self.st_blksize);
        LittleEndian::write_u64(&mut v[64..72], self.st_blocks);
        LittleEndian::write_u64(&mut v[72..80], self.st_atime);
        LittleEndian::write_u64(&mut v[80..88], self.st_atime_nsec);
        LittleEndian::write_u64(&mut v[88..96], self.st_mtime);
        LittleEndian::write_u64(&mut v[96..104], self.st_mtime_nsec);
        LittleEndian::write_u64(&mut v[104..112], self.st_ctime);
        LittleEndian::write_u64(&mut v[112..120], self.st_ctime_nsec);
        v
    }
}

impl From<Metadata> for Stats {
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

#[derive(Debug, Clone)]
pub enum FSEntry {
    FileEntry(File, Stats),
    DirEntry(Dir, Stats),
}

impl FSEntry {
    pub fn stat(&self) -> Vec<u8> {
        match self {
            FSEntry::DirEntry(_, s) => s.to_vec(),
            FSEntry::FileEntry(_, s) => s.to_vec(),
        }
    }
}

impl File {
    pub fn new(path: &path::Path) -> File {
        if path.is_dir() {
            panic!("bla");
        }
        let buf = fs::read(path).unwrap();
        File { data: buf }
    }

    pub fn size(&self) -> usize { self.data.len() }
}

impl<Idx> std::ops::Index<Idx> for File where Idx: std::slice::SliceIndex<[u8]> {
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.data[index]
    }
}

pub struct Filesystem {
    root_path: PathBuf,
    fs: HashMap<path::PathBuf, FSEntry>,
}

//TODO return hashmap and merge
fn readdir(root: &Path, dir: &Path, fs: &mut HashMap<PathBuf, FSEntry>) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let stats = entry.metadata().unwrap();
            let f = if path.is_dir() {
                info!("Loading directory {:?}", path);
                readdir(root, &path, fs)?;
                FSEntry::DirEntry(Dir {}, Stats::from(stats))
            } else {
                info!("Loading file {:?}", path);
                FSEntry::FileEntry(File::new(&path), Stats::from(stats))
            };
            fs.insert(path.strip_prefix(root).unwrap().to_path_buf(), f);
        }
    }
    Ok(())
}

impl Filesystem {
    pub fn new(root: &path::Path) -> Filesystem {
        let mut fs: HashMap<path::PathBuf, FSEntry> = HashMap::new();
        info!("Creating new filesystem with root dir {:?}", root);
        readdir(root, root, &mut fs).unwrap();
        Filesystem { root_path: root.to_path_buf(), fs }
    }

    pub fn root(&self) -> &Path { &self.root_path }

    pub fn get(&self, path: &Path) -> Option<FSEntry> {
        if let Ok(p) = path.strip_prefix("/") {
            Some(self.fs.get(p)?.clone())
        } else {
            None
        }
    }

    pub fn stat(&self, path: &Path) -> Result<Vec<u8>, IOErr> {
        if let Some(f) = self.get(path.strip_prefix("/").unwrap()) {
            Ok(f.stat())
        } else {
            Err(IOErr::ENOENT)
        }
    }
}
