//! Provides an in memory file system implementation with snapshot capabilities for usage by fuzzers.
//! TODO: Implement and re-usage of deleted memory
//! TODO: Improve error handling with more meaningful errors
//! TODO: Implement update of stat struct.
//! TODO: Consider memory mapping of files into ram
use std::cmp::min;
use std::fmt::{
    Debug,
    Formatter,
};
use std::fs::read;
use std::path::{
    Path,
    PathBuf,
};
use std::{
    fs,
    io,
};

use byteorder::ByteOrder;
use tracing::{
    debug,
    info,
    trace,
};

use data::{
    Data,
    DataSnapMarker,
};
use meta_info::{
    MetaInfo,
    MetaInfoSnapMarker,
};
pub use stat::Stat;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::filesystem::{FSEntry, FSId, FileId, Mountpoint};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::filesystem::meta_info::FSEntryContent::{
    File,
    Symlink,
};
use crate::emulator::system::filesystem::persistent::PersistendFS;

mod persistent;
mod stat;
mod tests;

#[derive(Debug, Copy, Clone, PartialEq)]
#[non_exhaustive]
pub enum FSError {
    NotFound,
    NoEntry,
    Not4kAlligned,
    NoAddress,
    IO,
    Invalid,
    Loop,
    Unknown,
    Exist,
    Busy,
    NotEmpty,
}

impl From<io::Error> for FSError {
    fn from(e: io::Error) -> Self {
        trace!("{}", e);
        FSError::IO
    }
}

pub type FSEntryId = usize;
type ContentId = usize;
type SnapId = usize;

#[derive(Debug)]
pub struct Filesystem {
    on_disk_root: Option<PathBuf>,
    mountpoints: Vec<Box<dyn Mountpoint>>,
}

impl Filesystem {
    pub fn new(size: usize) -> Self {
        let mountpoints: Vec<Box<dyn Mountpoint>> = vec![Box::new(PersistendFS::new(size))];
        Filesystem {
            on_disk_root: None,
            mountpoints,
        }
    }

    pub fn from_disk(
        size: usize,
        root: &Path,
    ) -> Result<Self, FSError> {
        let mut fs = Self::new(size);
        fs.on_disk_root = Some(root.to_path_buf());
        load_dir(&mut fs, root, root)?;
        Ok(fs)
    }

    pub fn on_disk_root(&self) -> Option<&Path> {
        if let Some(r) = &self.on_disk_root {
            Some(r.as_path())
        } else {
            None
        }
    }

    pub fn read_all(
        &self,
        file_id: FileId,
    ) -> Result<Vec<u8>, FSError> {
        let size = self.stat(file_id)?.st_size as usize;
        self.read(file_id, 0, size)
    }

    pub fn read_symlink(
        &self,
        file_id: FileId,
    ) -> Result<Option<PathBuf>, FSError> {
        let mountpoint = self
            .mountpoints
            .get(file_id.fs)
            .ok_or(FSError::Invalid)?
            .as_ref();
        let entry = mountpoint.get_entry(file_id.file)?;
        Ok(if let FSEntry::Symlink { contents } = entry {
            Some(contents)
        } else {
            None
        })
    }

    pub fn read(
        &self,
        file_id: FileId,
        pos: usize,
        len: usize,
    ) -> Result<Vec<u8>, FSError> {
        let mountpoint = self
            .mountpoints
            .get(file_id.fs)
            .ok_or(FSError::Invalid)?
            .as_ref();
        mountpoint.read(file_id.file, pos, len)
    }

    pub fn write(
        &mut self,
        file_id: FileId,
        pos: usize,
        data: &[u8],
    ) -> Result<usize, FSError> {
        let mountpoint = self
            .mountpoints
            .get_mut(file_id.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        mountpoint.write(file_id.file, pos, data)
    }

    pub fn truncate(
        &mut self,
        file_id: FileId,
        new_size: usize,
    ) -> Result<(), FSError> {
        let mountpoint = self
            .mountpoints
            .get_mut(file_id.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        mountpoint.truncate(file_id.file, new_size)
    }

    pub fn create_file(
        &mut self,
        path: &Path,
        stat: Option<Stat>,
    ) -> Result<FileId, FSError> {
        let d = self.resolve_last(path.parent().ok_or(FSError::NoEntry)?, false)?;
        let name = path.file_name().ok_or(FSError::Unknown)?.to_os_string();
        let mountpoint = self
            .mountpoints
            .get_mut(d.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(0, mountpoint.create_entry(d.file, name, FSEntry::File, stat)?))
    }

    pub fn create_file_with_contents(
        &mut self,
        path: &Path,
        data: &[u8],
        stat: Option<Stat>,
    ) -> Result<FileId, FSError> {
        let fse_id = self.create_file(path, stat)?;
        self.write(fse_id, 0, data)?;
        Ok(fse_id)
    }

    pub fn create_dir(
        &mut self,
        path: &Path,
        stat: Option<Stat>,
    ) -> Result<FileId, FSError> {
        let d = self.resolve_last(path.parent().ok_or(FSError::Invalid)?, false)?;
        let name = path.file_name().ok_or(FSError::Invalid)?.to_os_string();
        let mountpoint = self
            .mountpoints
            .get_mut(d.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(0, mountpoint.create_entry(d.file, name, FSEntry::Dir, stat)?))
    }

    pub fn create_symlink(
        &mut self,
        path: &Path,
        target: PathBuf,
        stat: Option<Stat>,
    ) -> Result<FileId, FSError> {
        let d = self.resolve_last(path.parent().ok_or(FSError::Invalid)?, false)?;
        let name = path.file_name().ok_or(FSError::Invalid)?.to_os_string();
        let mountpoint = self
            .mountpoints
            .get_mut(d.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(0, mountpoint.create_entry(d.file, name, FSEntry::Symlink { contents: target }, stat)?))
    }

    pub fn delete(
        &mut self,
        path: &Path,
    ) -> Result<(), FSError> {
        // FIXME: Implement separate unlink function that just removes the fs entry
        //        from its parent, so open fds can still access the entry.
        //        The last remaining fd can then do the final delete.
        //        (How do we track which fd is the last one?)
        //        This also has to be tracked across snapshots.
        let d = self.resolve_last(path.parent().ok_or(FSError::Invalid)?, false)?;
        let name = path.file_name().ok_or(FSError::Invalid)?.to_os_string();
        let mountpoint = self
            .mountpoints
            .get_mut(d.fs)
            .ok_or(FSError::Invalid)?
            .as_mut();
        mountpoint.delete(d.file, name)
    }

    pub fn stat(
        &self,
        file_id: FileId,
    ) -> Result<&Stat, FSError> {
        let mountpoint = self
            .mountpoints
            .get(file_id.fs)
            .ok_or(FSError::Invalid)?
            .as_ref();
        mountpoint.stat(file_id.file)
    }

    /// This function resolves the supplied path to a list of FileId. After the lookup,
    /// the file or dirs in the path can be directly referenced by using the returned FileIds.
    /// The FileIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FileId of each element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_path(
        &self,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<Vec<FileId>, FSError> {
        self.resolve_path_relative(None, path, follow_symlink)
    }

    pub fn resolve_path_relative(
        &self,
        dir: Option<FileId>,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<Vec<FileId>, FSError> {
        trace!("open at: {:?} {:?} follow_symlink: {}", dir, path, follow_symlink);
        let mut loopcount = 30u8;
        let mut fs_id = match dir {
            None => 0,
            Some(FileId { fs, .. }) => fs,
        };
        let mut mountpoint = self
            .mountpoints
            .get(fs_id)
            .ok_or(FSError::Invalid)?
            .as_ref();
        let dir: Option<FSEntryId> = dir.map(|x| x.file);
        let (ids, mut remain) = mountpoint.resolve_path(dir, path)?;
        let mut result: Vec<FileId> = ids.into_iter().map(|id| FileId::new(id, fs_id)).collect();
        while remain.is_some() {
            trace!("remain: {:?}", remain);
            let last = result.pop().expect("Empty paths cannot occur");
            match mountpoint.get_entry(last.file)? {
                FSEntry::Symlink { contents } => {
                    let symlinkdir = if contents.is_relative() {
                        Some(*result.last().unwrap())
                    } else {
                        None
                    };
                    let target = self.resolve_symlink(symlinkdir, FileId::new(last.fs, last.file), &mut loopcount)?;
                    mountpoint = self
                        .mountpoints
                        .get(target.fs)
                        .ok_or(FSError::Unknown)?
                        .as_ref();
                    result.push(target);
                    let (mut a, r) = mountpoint.resolve_path(Some(target.file), remain.expect("").as_path())?;
                    let mut b = a.into_iter().map(|id| FileId::new(target.fs, id)).collect();
                    result.append(&mut b);
                    remain = r;
                }
                _ => {}
            }
        }
        while follow_symlink
            && self
                .read_symlink(*result.last().expect("Empty paths cannot occur"))?
                .is_some()
        {
            if loopcount == 0 {
                Err(FSError::Loop)?;
            }
            let link = result.pop().expect("Empty paths cannot occur");
            let symlinkdir = if self.read_symlink(link)?.unwrap().is_relative() {
                Some(*result.last().unwrap())
            } else {
                None
            };
            let target = self.resolve_symlink(symlinkdir, link, &mut loopcount)?;
            result.push(target);
        }
        Ok(result)
    }

    /// This function resolves the supplied path to an FileId. After the lookup,
    /// the file or dir at the end of the path can be directly referenced by using the returned FileIds.
    /// The FileIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FileId of the last element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_last(
        &self,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<FileId, FSError> {
        self.resolve_last_relative(None, path, follow_symlink)
    }

    /// This function resolves the supplied path to an FileId. The lookup is performed
    /// relative to the supplied directory id, if the id is not None.
    /// See also [Filesystem::resolve_last()]
    ///
    /// returns: The FileId of the last element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_last_relative(
        &self,
        dir: Option<FileId>,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<FileId, FSError> {
        Ok(*self
            .resolve_path_relative(dir, path, follow_symlink)?
            .last()
            .expect("Empty paths cannot occur"))
    }

    fn resolve_symlink(
        &self,
        dir: Option<FileId>,
        link: FileId,
        loopcount: &mut u8,
    ) -> Result<FileId, FSError> {
        *loopcount -= 1;
        let target = self
            .read_symlink(link)?
            .expect("link target cannot be None at this point");
        let (mut fs_id, mut dir) = if target.is_relative() {
            (dir.unwrap().fs, dir.map(|x| x.file))
        } else {
            (0, None)
        };
        let mut mountpoint = self
            .mountpoints
            .get(fs_id)
            .ok_or(FSError::Invalid)?
            .as_ref();
        let (mut ids, mut remain) = mountpoint.resolve_path(dir, &target)?;
        while remain.is_some() && *loopcount > 0 {
            let link = ids.pop().expect("Empty paths cannot occur");
            let target = self
                .read_symlink(FileId::new(0, link))?
                .expect("link target cannot be None at this point");
            dir = if target.is_relative() {
                ids.last().copied()
            } else {
                None
            };
            remain = Some(target.join(remain.expect("").as_path()));
            (ids, remain) = mountpoint.resolve_path(dir, remain.expect("").as_path())?;
            *loopcount -= 1;
        }

        if remain.is_some() {
            Err(FSError::Loop)
        } else {
            Ok(FileId::new(0, *ids.last().expect("Empty paths cannot occur")))
        }
    }

    fn print_tree(&self) {
        todo!();
    }
}

impl Restorable for Filesystem {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        for mp in self.mountpoints.iter_mut() {
            let mountpoint = mp.as_mut();
            mountpoint.take_snapshot()?;
        }
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        for mp in self.mountpoints.iter_mut() {
            let mountpoint = mp.as_mut();
            mountpoint.discard_current_and_restore_previous_snapshot()?;
        }
        Ok(())
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        for mp in self.mountpoints.iter_mut() {
            let mountpoint = mp.as_mut();
            mountpoint.restore_snapshot()?;
        }
        Ok(())
    }
}

fn load_dir(
    fs: &mut Filesystem,
    dir: &Path,
    root: &Path,
) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let stat = entry.metadata().unwrap();
            if path.is_symlink() {
                trace!("symlink {}", path.display());
                let link = fs::read_link(entry.path())?;
                fs.create_symlink(&Path::new("/").join(path.strip_prefix(root).unwrap()), link, Some(stat.into()))
                    .unwrap();
            } else if path.is_dir() {
                trace!("dir {}", path.display());
                fs.create_dir(&Path::new("/").join(path.strip_prefix(root).unwrap()), Some(stat.into()))
                    .unwrap();
                load_dir(fs, &path, root)?;
            } else if path.is_file() {
                trace!("file {}", path.display());
                let data = read(entry.path())?;
                fs.create_file_with_contents(
                    &Path::new("/").join(path.strip_prefix(root).unwrap()),
                    data.as_slice(),
                    Some(stat.into()),
                )
                .unwrap();
            } else {
                info!("ignore file {}: unsuported file type", path.strip_prefix(root).unwrap().display());
            };
        }
    }
    Ok(())
}

const MASK4K: usize = 4 * (1 << 10) - 1;

fn is_4k_aligned(val: usize) -> bool {
    val & MASK4K == 0
}

fn align_4k(val: usize) -> usize {
    (val + MASK4K) & !MASK4K
}

fn align_4k_down(val: usize) -> usize {
    val & !MASK4K
}
