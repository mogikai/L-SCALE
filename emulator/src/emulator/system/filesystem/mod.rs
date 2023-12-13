//! Provides an in memory file system implementation with snapshot capabilities for usage by fuzzers.
//! TODO: Implement and re-usage of deleted memory
//! TODO: Improve error handling with more meaningful errors
//! TODO: Implement update of stat struct.
//! TODO: Consider memory mapping of files into ram
use std::cmp::min;
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
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::filesystem::meta_info::FSEntryContent::{
    File,
    Symlink,
};

mod block;
mod data;
mod meta_info;
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
    data: Data,
    meta_info: MetaInfo,
    snap_markers: Vec<SnapMarker>,
    current_size: usize,
    max_size: usize,
    current_snap: usize,
}

impl Filesystem {
    pub fn new(size: usize) -> Self {
        Filesystem {
            on_disk_root: None,
            data: Data::new(),
            snap_markers: Vec::new(),
            meta_info: MetaInfo::new(),
            current_size: 0,
            max_size: size,
            current_snap: 0,
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
        file_id: FSEntryId,
    ) -> Result<Vec<u8>, FSError> {
        let size = self.stat(file_id)?.st_size as usize;
        self.read(file_id, 0, size)
    }

    pub fn read_symlink(
        &self,
        file_id: FSEntryId,
    ) -> Result<Option<PathBuf>, FSError> {
        let entry = self.meta_info.most_recent_entry(file_id)?;
        Ok(if let Symlink { contents } = &entry.contents {
            Some(contents.clone())
        } else {
            None
        })
    }

    pub fn read(
        &self,
        file_id: FSEntryId,
        pos: usize,
        len: usize,
    ) -> Result<Vec<u8>, FSError> {
        let entry = self.meta_info.most_recent_entry(file_id)?;
        if let File { contents } = &entry.contents {
            if pos > entry.stat.st_size as usize {
                return Err(FSError::NoAddress);
            }
            let len = min(len, entry.stat.st_size as usize - pos);
            let mut read = 0;
            let start_block = contents
                .range(..=pos)
                .last()
                .expect("Must exist since pos < size");
            let mut buffer = Vec::with_capacity(len);

            for block in contents.range(start_block.start..pos + len) {
                read += self
                    .data
                    .read_block(pos + read, block, buffer.spare_capacity_mut())?;
                unsafe {
                    buffer.set_len(read);
                }
            }
            Ok(buffer)
        } else {
            trace!("cant read: {:?}", entry.contents);
            Err(FSError::Invalid)
        }
    }

    pub fn write(
        &mut self,
        file_id: FSEntryId,
        pos: usize,
        data: &[u8],
    ) -> Result<usize, FSError> {
        let entry = self.meta_info.most_recent_entry_mut(file_id)?;
        let size = entry.stat.st_size as usize;
        if let File { contents } = &mut entry.contents {
            if (pos + data.len()).saturating_sub(size) > self.max_size - self.current_size {
                // No space available
                debug!(
                    "NO SPACE! remaining: {}, want: {}",
                    self.max_size - self.current_size,
                    (pos + data.len()).saturating_sub(size)
                );
                Err(FSError::Unknown)?
            }

            let start_block = contents.range(..=pos).last();
            let mut written = 0;
            if let Some(start_block) = start_block {
                if start_block.contains(pos) {
                    for block_start in contents
                        .range(start_block.start..pos + data.len())
                        .map(|b| b.start)
                        .collect::<Vec<_>>()
                    {
                        written +=
                            self.data
                                .write_to_block_cow(pos + written, &data[written..], contents, block_start)?;
                    }
                }
            }

            // Write the remaining data to a new block.
            if data.len() > written {
                self.data
                    .new_block(pos + written, contents, &data[written..])?;
                written = data.len();
            }

            let additional_size = (pos + written).saturating_sub(size);
            entry.stat.st_size += additional_size as u64;
            self.current_size += additional_size;

            Ok(written)
        } else {
            Err(FSError::Unknown)
        }
    }

    pub fn truncate(
        &mut self,
        file_id: FSEntryId,
        new_size: usize,
    ) -> Result<(), FSError> {
        let entry = &self.meta_info.most_recent_entry(file_id)?;
        let size = entry.stat.st_size as usize;
        let size = if let File { .. } = &entry.contents {
            Ok(size)
        } else {
            Err(FSError::Unknown)
        }?;

        if new_size > size {
            let buf = vec![0; new_size - size];
            self.write(file_id, new_size - size, &buf)?;
            Ok(())
        } else {
            let entry = self.meta_info.most_recent_entry_mut(file_id)?;
            if let File { contents } = &mut entry.contents {
                self.current_size -= entry.stat.st_size as usize - new_size;
                entry.stat.st_size = new_size as u64;
                self.data
                    .save_free_blocks(contents.split_off(&new_size).into_iter());
                if let Some(block) = contents.iter().last().cloned() {
                    self.data.write_to_block_cow(
                        new_size,
                        vec![0; block.end() - new_size].as_slice(),
                        contents,
                        block.start,
                    )?;
                }
                Ok(())
            } else {
                Err(FSError::Unknown)
            }
        }
    }

    pub fn create_file(
        &mut self,
        path: &Path,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        self.meta_info.create_file(path, stat)
    }

    pub fn create_file_with_contents(
        &mut self,
        path: &Path,
        data: &[u8],
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        let fse_id = self.create_file(path, stat)?;
        self.write(fse_id, 0, data)?;
        Ok(fse_id)
    }

    pub fn create_dir(
        &mut self,
        path: &Path,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        self.meta_info.create_dir(path, stat)
    }

    pub fn create_symlink(
        &mut self,
        path: &Path,
        target: PathBuf,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        self.meta_info.create_link(path, stat, target)
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
        self.meta_info.delete(path)
    }

    pub fn stat(
        &self,
        file_id: FSEntryId,
    ) -> Result<&Stat, FSError> {
        Ok(&self.meta_info.most_recent_entry(file_id)?.stat)
    }

    pub fn path_from(
        &self,
        file_id: FSEntryId,
    ) -> Result<PathBuf, FSError> {
        self.meta_info.path_from(file_id)
    }

    /// This function resolves the supplied path to a list of FSEntryId. After the lookup,
    /// the file or dirs in the path can be directly referenced by using the returned FSEntryIds.
    /// The FSEntryIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FSEntryId of each element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_path(
        &self,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<Vec<FSEntryId>, FSError> {
        self.resolve_path_relative(None, path, follow_symlink)
    }

    pub fn resolve_path_relative(
        &self,
        dir: Option<FSEntryId>,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<Vec<FSEntryId>, FSError> {
        trace!("open at: {:?} {:?} follow_symlink: {}", dir, path, follow_symlink);
        let mut loopcount = 30u8;
        let (mut ids, mut remain) = self.meta_info.resolve_path(dir, path)?;
        while remain.is_some() {
            trace!("remain: {:?}", remain);
            let link = ids.pop().expect("Empty paths cannot occur");
            let result = self.resolve_symlink(ids.last().copied(), link, &mut loopcount)?;
            ids.push(result);
            let (mut a, r) = self
                .meta_info
                .resolve_path(Some(result), remain.expect("").as_path())?;
            ids.append(&mut a);
            remain = r;
        }
        while follow_symlink
            && self
                .read_symlink(*ids.last().expect("Empty paths cannot occur"))?
                .is_some()
        {
            if loopcount == 0 {
                Err(FSError::Loop)?;
            }
            let link = ids.pop().expect("Empty paths cannot occur");
            let result = self.resolve_symlink(ids.last().copied(), link, &mut loopcount)?;
            ids.push(result);
        }
        Ok(ids)
    }

    /// This function resolves the supplied path to an FSEntryId. After the lookup,
    /// the file or dir at the end of the path can be directly referenced by using the returned FSEntryIds.
    /// The FSEntryIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FSEntryId of the last element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_last(
        &self,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<FSEntryId, FSError> {
        self.resolve_last_relative(None, path, follow_symlink)
    }

    /// This function resolves the supplied path to an FSEntryId. The lookup is performed
    /// relative to the supplied directory id, if the id is not None.
    /// See also [Filesystem::resolve_last()]
    ///
    /// returns: The FSEntryId of the last element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_last_relative(
        &self,
        dir: Option<FSEntryId>,
        path: &Path,
        follow_symlink: bool,
    ) -> Result<FSEntryId, FSError> {
        Ok(*self
            .resolve_path_relative(dir, path, follow_symlink)?
            .last()
            .expect("Empty paths cannot occur"))
    }

    fn resolve_symlink(
        &self,
        dir: Option<FSEntryId>,
        link: FSEntryId,
        loopcount: &mut u8,
    ) -> Result<FSEntryId, FSError> {
        *loopcount -= 1;
        let target = self
            .read_symlink(link)?
            .expect("link target cannot be None at this point");
        let mut dir = if target.is_relative() { dir } else { None };
        let (mut ids, mut remain) = self.meta_info.resolve_path(dir, &target)?;
        while remain.is_some() && *loopcount > 0 {
            let link = ids.pop().expect("Empty paths cannot occur");
            let target = self
                .read_symlink(link)?
                .expect("link target cannot be None at this point");
            dir = if target.is_relative() {
                ids.last().copied()
            } else {
                None
            };
            remain = Some(target.join(remain.expect("").as_path()));
            (ids, remain) = self
                .meta_info
                .resolve_path(dir, remain.expect("").as_path())?;
            *loopcount -= 1;
        }

        if remain.is_some() {
            Err(FSError::Loop)
        } else {
            Ok(*ids.last().expect("Empty paths cannot occur"))
        }
    }

    fn print_tree(&self) {
        self.meta_info.print_tree(self.meta_info.root(), None);
    }
}

impl Restorable for Filesystem {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        assert_eq!(self.snap_markers.len(), self.current_snap);
        self.snap_markers.push(SnapMarker {
            data_marker: self.data.snapshot(),
            meta_info_marker: self.meta_info.snapshot(),
            current_size: self.current_size,
        });
        self.current_snap += 1;
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        if self.current_snap == 0 {
            Err(FSError::Unknown)?
        }
        let snap_marker = self
            .snap_markers
            .pop()
            .expect("Must exist since current_snap > 0");
        self.meta_info
            .restore_and_discard(snap_marker.meta_info_marker);
        self.data.restore_and_discard(snap_marker.data_marker);
        self.current_size = snap_marker.current_size;
        self.current_snap -= 1;
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        if self.current_snap == 0 {
            Err(FSError::Unknown)?
        }
        let snap_marker = self
            .snap_markers
            .last()
            .expect("Must exist since current_snap > 0");
        self.meta_info.restore(snap_marker.meta_info_marker);
        self.data.restore(snap_marker.data_marker);
        self.current_size = snap_marker.current_size;
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
                // FSEntry::DirEntry(Dir {}, Stats::from(stats))
            } else if path.is_file() {
                trace!("file {}", path.display());
                let data = read(entry.path())?;
                fs.create_file_with_contents(
                    &Path::new("/").join(path.strip_prefix(root).unwrap()),
                    data.as_slice(),
                    Some(stat.into()),
                )
                .unwrap();
                // FSEntry::FileEntry(File::new(&path), Stats::from(stats))
            } else {
                info!("ignore file {}: unsuported file type", path.strip_prefix(root).unwrap().display());
            };
            // println!("{:?}", Path::new("/").join(path.strip_prefix(root).unwrap().to_path_buf()));
        }
    }
    Ok(())
}

#[derive(Debug, Copy, Clone)]
struct SnapMarker {
    data_marker: DataSnapMarker,
    meta_info_marker: MetaInfoSnapMarker,
    current_size: usize,
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
