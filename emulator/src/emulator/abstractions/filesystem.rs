use mogi_system::errors::filesystem::FSError;
use std::ffi::OsString;
use std::fmt::Debug;
use std::ops::AddAssign;
use std::path::{
    Path,
    PathBuf,
};

use crate::emulator::system::filesystem::Stat;
use mogi_system::components::restore::Restorable;

#[derive(Default, Copy, Clone, Debug, PartialEq)]
pub struct FSId(usize);

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FSEntryId(usize);

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct FileId {
    pub fs: FSId,
    pub file: FSEntryId,
}

impl From<usize> for FSId {
    fn from(value: usize) -> Self {
        FSId(value)
    }
}

impl From<usize> for FSEntryId {
    fn from(value: usize) -> Self {
        FSEntryId(value)
    }
}

impl From<FSId> for usize {
    fn from(value: FSId) -> Self {
        value.0
    }
}

impl From<FSEntryId> for usize {
    fn from(value: FSEntryId) -> Self {
        value.0
    }
}

impl AddAssign<usize> for FSEntryId {
    fn add_assign(
        &mut self,
        rhs: usize,
    ) {
        *self = (self.0 + rhs).into();
    }
}

impl FileId {
    pub fn new(
        fs: FSId,
        file: FSEntryId,
    ) -> Self {
        FileId { fs, file }
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

    fn resolve_name(
        &self,
        parent: FSEntryId,
        entry: FSEntryId,
    ) -> Result<OsString, FSError>;

    fn resolve_children(
        &self,
        parent: FSEntryId,
    ) -> Result<Vec<FSEntryId>, FSError>;

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

    fn set_mountpoint(
        &mut self,
        fsentry: FSEntryId,
        fsid: FSId,
    ) -> Result<(), FSError>;
    /* TODO breitensuche
    fn breadth_first_visitor(&mut self, visitor);
     */
}
