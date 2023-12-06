use crate::emulator::abstractions::restore::Restorable;
use std::ffi::OsString;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

use crate::emulator::system::filesystem::{FSEntryId, FSError, Stat};

pub type FSId = usize;
//type FSEntryId = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FileId {
    pub fs: FSId,
    pub file: FSEntryId,
}

impl FileId {
    pub fn new(
        fs: FSId,
        file: FSEntryId,
    ) -> Self {
        FileId { file, fs }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FSEntry {
    Dir,
    File,
    Symlink { contents: PathBuf },
    Mount { fs: FSId },
}

pub trait Mountpoint: Restorable + Debug {
    fn resolve_path(
        &self,
        parent: Option<FSEntryId>,
        path: &Path,
    ) -> Result<(Vec<FSEntryId>, Option<PathBuf>), FSError>;
    fn get_entry(
        &self,
        entry: FSEntryId,
    ) -> Result<FSEntry, FSError>;
    fn read(
        &self,
        entry: FSEntryId,
        pos: usize,
        len: usize,
    ) -> Result<Vec<u8>, FSError>;
    fn write(
        &mut self,
        entry: FSEntryId,
        pos: usize,
        data: &[u8],
    ) -> Result<usize, FSError>;
    fn truncate(
        &mut self,
        file_id: FSEntryId,
        new_size: usize,
    ) -> Result<(), FSError>;

    fn stat(
        &self,
        entry: FSEntryId,
    ) -> Result<&Stat, FSError>;
    fn create_entry(
        &mut self,
        dir: FSEntryId,
        name: OsString,
        entry: FSEntry,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError>;
    fn delete(
        &mut self,
        dir: FSEntryId,
        name: OsString,
    ) -> Result<(), FSError>;
    /* TODO breitensuche
    fn breadth_first_visitor(&mut self, visitor);
     */
}
