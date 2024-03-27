//! Provides an in memory file system implementation with snapshot capabilities for usage by fuzzers.
//! TODO: Implement and re-usage of deleted memory
//! TODO: Improve error handling with more meaningful errors
//! TODO: Implement update of stat struct.
//! TODO: Consider memory mapping of files into ram
use std::cmp::min;
use std::ffi::OsString;
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
use mogi_system::errors::filesystem::FSError;
use tracing::{
    debug,
    info,
    trace,
};

pub use stat::Stat;

use crate::emulator::abstractions::filesystem::{
    FSEntry,
    FSId,
    FileId,
    Mountpoint,
};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::filesystem::persistent::PersistendFS;
use mogi_system::errors::generic::GenericError;

mod persistent;
mod stat;
mod tests;

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
            .get(usize::from(file_id.fs))
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
            .get(usize::from(file_id.fs))
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
            .get_mut(usize::from(file_id.fs))
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
            .get_mut(usize::from(file_id.fs))
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
            .get_mut(usize::from(d.fs))
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(d.fs, mountpoint.create_entry(d.file, name, FSEntry::File, stat)?))
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
            .get_mut(usize::from(d.fs))
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(d.fs, mountpoint.create_entry(d.file, name, FSEntry::Dir, stat)?))
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
            .get_mut(usize::from(d.fs))
            .ok_or(FSError::Invalid)?
            .as_mut();
        Ok(FileId::new(
            d.fs,
            mountpoint.create_entry(d.file, name, FSEntry::Symlink { contents: target }, stat)?,
        ))
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
            .get_mut(usize::from(d.fs))
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
            .get(usize::from(file_id.fs))
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

        let mut result = vec![dir.unwrap_or(FileId::default())];
        let mut remain = Some(path.to_path_buf());
        let mut last_was_symlink = false;

        while remain.is_some() || last_was_symlink {
            last_was_symlink = false;

            // result.last() will be used in each iteration to continue off from
            let mut last = *result.last().expect("Empty paths cannot occur");

            let mountpoint = self
                .mountpoints
                .get(usize::from(last.fs))
                .ok_or(FSError::Invalid)?
                .as_ref();

            // resolve as much as possible with current mountpoint for next iteration in case
            // there is a remaining path
            if let Some(some_remain) = &remain {
                let (ids, mut new_remain) = mountpoint.resolve_path(Some(last.file), some_remain)?;
                result.pop();
                result.extend(ids.into_iter().map(|id| FileId::new(last.fs, id)));
                remain = new_remain;
                last = *result.last().expect("Empty paths cannot occur");
            }

            match mountpoint.get_entry(last.file)? {
                // if the (new) last entry is a symlink, follow
                FSEntry::Symlink { contents } => {
                    if !follow_symlink {
                        break;
                    }

                    let symlinkdir = if contents.is_relative() {
                        Some(*result.get(result.len() - 2).ok_or(FSError::Invalid)?)
                    } else {
                        None
                    };

                    let target = self.resolve_symlink(symlinkdir, last, &mut loopcount)?;

                    if loopcount == 0 {
                        Err(FSError::Loop)?
                    }

                    last_was_symlink = true;

                    // replace result.last() by symlink target
                    result.pop();
                    result.push(target);
                }

                // if the (new) last entry is a mount, continue with child mount
                FSEntry::Mount { fs } => {
                    let child_mountpoint = self
                        .mountpoints
                        .get(usize::from(fs))
                        .ok_or(FSError::Invalid)?
                        .as_ref();
                    let (child_root, _) = child_mountpoint.resolve_path(None, Path::new("/"))?;

                    // replace result.last() by mount target (i.e. its root)
                    result.pop();
                    result.push(FileId::new(fs, *child_root.first().ok_or(FSError::Invalid)?));
                }
                _ => {}
            }
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

    /// This function mounts a given Mountpoint to a Path. Path has to resolve
    /// to a FileId with FSEntry equal to FSEntry::Dir which will be changed to
    /// FSEntry::Mount so existing files will be lost.
    ///
    /// returns: The FSId of the newly mounted Mountpoint. If the Path can't be
    /// resolved, an error is returned.
    pub fn mount(
        &mut self,
        path: &Path,
        mountpoint: Box<dyn Mountpoint>,
    ) -> Result<FSId, FSError> {
        let new_fs = self.mountpoints.len().into();
        let parent_dir = self.resolve_last(path.parent().ok_or(FSError::NoEntry)?, false)?;

        // parent mount is equal to filesystem of dir
        let dir = self.resolve_last(path, false)?;
        let parent_mountpoint = self
            .mountpoints
            .get_mut(usize::from(dir.fs))
            .ok_or(FSError::Invalid)?
            .as_mut();

        // change parent_dir type to mount (delete then recreate)
        let dir_name: OsString = path.file_name().ok_or(FSError::NoEntry)?.into();
        let stat = parent_mountpoint.stat(parent_dir.file)?.clone();
        parent_mountpoint.delete(parent_dir.file, dir_name.clone())?;
        parent_mountpoint.create_entry(parent_dir.file, dir_name, FSEntry::Mount { fs: new_fs }, Some(stat));

        // TODO mountpoint.set_mountpoint(parent_dir.file, fsid); (not necessary?)
        self.mountpoints.push(mountpoint);
        Ok(new_fs)
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
        let (mut fs, mut dir) = if target.is_relative() {
            (dir.unwrap().fs, dir.map(|x| x.file))
        } else {
            (FSId::default(), None)
        };
        let mut mountpoint = self
            .mountpoints
            .get(usize::from(fs))
            .ok_or(FSError::Invalid)?
            .as_ref();
        let (mut ids, mut remain) = mountpoint.resolve_path(dir, &target)?;
        while remain.is_some() && *loopcount > 0 {
            let link = ids.pop().expect("Empty paths cannot occur");
            let target = self
                .read_symlink(FileId::new(fs, link))?
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
            Ok(FileId::new(fs, *ids.last().expect("Empty paths cannot occur")))
        }
    }

    fn print_tree(&self) -> Result<(), FSError> {
        let root = self.resolve_last(Path::new("/"), false)?;

        // stack (depth, parent, file)
        let mut remaining_files: Vec<(u32, Option<FileId>, FileId)> = vec![(0, None, root)];

        // as long as files are on the stack
        while let Some(file) = remaining_files.pop() {
            let (depth, parent, file) = file;

            // get mountpoint and name of file
            let mountpoint = self
                .mountpoints
                .get(usize::from(file.fs))
                .ok_or(FSError::NotFound)?;
            let entry = mountpoint.get_entry(file.file)?;
            let name = if let Some(parent) = parent {
                mountpoint.resolve_name(parent.file, file.file)?
            } else {
                "/".into()
            };

            match entry {
                // file: just print
                FSEntry::File => println!("{:>pad$} {:#?} (f)", "", name, pad = depth as usize * 2),

                // dir: push children on stack
                FSEntry::Dir => {
                    let children = mountpoint.resolve_children(file.file)?;
                    remaining_files.extend(
                        children
                            .iter()
                            .map(|id| (depth + 1, Some(file), FileId::new(file.fs, *id))),
                    );

                    println!("{:>pad$} {:#?} (d)", "", name, pad = depth as usize * 2);
                }

                // mount: push children of root of new file system on stack
                FSEntry::Mount { fs } => {
                    let child_mountpoint = self
                        .mountpoints
                        .get(usize::from(fs))
                        .ok_or(FSError::NotFound)?;
                    let (root, _) = child_mountpoint.resolve_path(None, Path::new("/"))?;
                    let children = child_mountpoint.resolve_children(*root.last().ok_or(FSError::NotFound)?)?;
                    remaining_files.extend(children.iter().map(|id| {
                        (
                            depth + 1,
                            Some(FileId::new(fs, *root.first().expect("Root can't be None"))),
                            FileId::new(fs, *id),
                        )
                    }));

                    println!("{:>pad$} {:#?} (m; fs={:?})", "", name, fs, pad = depth as usize * 2);
                }

                // symlink: don't resolve; just print
                FSEntry::Symlink { contents } => {
                    println!("{:>pad$} {:#?} (s)", "", name, pad = depth as usize * 2);
                }
            }
        }

        Ok(())
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
