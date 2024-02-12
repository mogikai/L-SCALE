mod block;
mod data;
mod inodes;

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::filesystem::{FSEntry, FSId, Mountpoint};
use crate::emulator::abstractions::restore::Restorable;
use std::cmp::min;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tracing::{trace, debug};

use crate::emulator::system::filesystem::persistent::data::{Data, DataSnapMarker};
use crate::emulator::system::filesystem::persistent::inodes::{FSContent, Inode, Inodes, MetaInfoSnapMarker};
use crate::emulator::system::filesystem::stat::Stat;
use crate::emulator::system::filesystem::{FSEntryId, FSError};

#[derive(Debug)]
pub struct PersistendFS {
    inodes: Inodes,
    data: Data,
    max_size: usize,
    current_size: usize,
    root_node: usize,
    snap_markers: Vec<SnapMarker>,
    current_snap: usize,
}

impl PersistendFS {
    pub fn new(size: usize) -> Self {
        let mut inodes = Inodes::new();
        inodes
            .add_entry(
                0,
                Inode::new(
                    FSEntry::Dir,
                    Stat::default(),
                    Some(FSContent::Dir {
                        entries: BTreeMap::new(),
                    }),
                ),
            )
            .unwrap();

        //        let root_node = inodes.create_dir("/".as_ref(), None).unwrap();
        let data = Data::new();
        PersistendFS {
            inodes,
            data,
            root_node: 0,
            max_size: size,
            current_size: 0,
            current_snap: 0,
            snap_markers: Vec::new(),
        }
    }
    pub(crate) fn print_tree(&self) {
        self.inodes.print_tree(0, None);
    }
}

impl Mountpoint for PersistendFS {
    fn resolve_path(
        &self,
        parent: Option<FSEntryId>,
        path: &Path,
    ) -> Result<(Vec<FSEntryId>, Option<PathBuf>), FSError> {
        self.inodes
            .resolve_path(parent.unwrap_or(self.root_node), path)
    }

    fn get_entry(
        &self,
        entry: FSEntryId,
    ) -> Result<FSEntry, FSError> {
        Ok(self.inodes.most_recent_entry(entry)?.file_type.clone())
    }

    fn read(
        &self,
        entry: FSEntryId,
        pos: usize,
        len: usize,
    ) -> Result<Vec<u8>, FSError> {
        let entry = self.inodes.most_recent_entry(entry)?;
        if let Some(FSContent::File { blocks }) = &entry.content {
            if pos > entry.stat.st_size as usize {
                return Err(FSError::NoAddress);
            }
            let len = min(len, entry.stat.st_size as usize - pos);
            let mut read = 0;
            let start_block = blocks
                .range(..=pos)
                .last()
                .expect("Must exist since pos < size");
            let mut buffer = Vec::with_capacity(len);

            for block in blocks.range(start_block.start..pos + len) {
                read += self
                    .data
                    .read_block(pos + read, block, buffer.spare_capacity_mut())?;
                unsafe {
                    buffer.set_len(read);
                }
            }
            Ok(buffer)
        } else {
            trace!("cant read: {:?}", entry.content);
            Err(FSError::Invalid)
        }
    }

    fn write(
        &mut self,
        entry: FSEntryId,
        pos: usize,
        data: &[u8],
    ) -> Result<usize, FSError> {
        let entry = self.inodes.most_recent_entry_mut(entry)?;
        let size = entry.stat.st_size as usize;
        if let Some(FSContent::File { blocks }) = &mut entry.content {
            if (pos + data.len()).saturating_sub(size) > self.max_size - self.current_size {
                // No space available
                debug!(
                    "NO SPACE! remaining: {}, want: {}",
                    self.max_size - self.current_size,
                    (pos + data.len()).saturating_sub(size)
                );
                Err(FSError::Unknown)?
            }

            let start_block = blocks.range(..=pos).last();
            let mut written = 0;
            if let Some(start_block) = start_block {
                if start_block.contains(pos) {
                    for block_start in blocks
                        .range(start_block.start..pos + data.len())
                        .map(|b| b.start)
                        .collect::<Vec<_>>()
                    {
                        written +=
                            self.data
                                .write_to_block_cow(pos + written, &data[written..], blocks, block_start)?;
                    }
                }
            }

            // Write the remaining data to a new block.
            if data.len() > written {
                self.data
                    .new_block(pos + written, blocks, &data[written..])?;
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

    fn truncate(
        &mut self,
        file_id: FSEntryId,
        new_size: usize,
    ) -> Result<(), FSError> {
        let entry = &self.inodes.most_recent_entry(file_id)?;
        let size = entry.stat.st_size as usize;
        let size = if FSEntry::File == entry.file_type {
            Ok(size)
        } else {
            Err(FSError::Unknown)
        }?;

        if new_size > size {
            let buf = vec![0; new_size - size];
            self.write(file_id, new_size - size, &buf)?;
            Ok(())
        } else {
            let entry = self.inodes.most_recent_entry_mut(file_id)?;
            if let Some(FSContent::File { blocks }) = &mut entry.content {
                self.current_size -= entry.stat.st_size as usize - new_size;
                entry.stat.st_size = new_size as u64;
                self.data
                    .save_free_blocks(blocks.split_off(&new_size).into_iter());
                if let Some(block) = blocks.iter().last().cloned() {
                    self.data.write_to_block_cow(
                        new_size,
                        vec![0; block.end() - new_size].as_slice(),
                        blocks,
                        block.start,
                    )?;
                }
                Ok(())
            } else {
                Err(FSError::Unknown)
            }
        }
    }

    fn stat(
        &self,
        entry: FSEntryId,
    ) -> Result<&Stat, FSError> {
        Ok(&self.inodes.most_recent_entry(entry)?.stat)
    }

    fn create_entry(
        &mut self,
        dir: FSEntryId,
        name: OsString,
        entry: FSEntry,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        self.inodes.new_entry(dir, name, entry, stat)
    }

    fn delete(
        &mut self,
        dir: FSEntryId,
        name: OsString,
    ) -> Result<(), FSError> {
        self.inodes.delete(dir, name)
    }

    fn set_mountpoint(&mut self, fsentry: FSEntryId, fsid: FSId) -> Result<(), FSError> {
        todo!()
    }
}

impl Restorable for PersistendFS {
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        assert_eq!(self.snap_markers.len(), self.current_snap);
        self.snap_markers.push(SnapMarker {
            data_marker: self.data.snapshot(),
            meta_info_marker: self.inodes.snapshot(),
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
        self.inodes
            .restore_and_discard(snap_marker.meta_info_marker);
        self.data.restore_and_discard(snap_marker.data_marker);
        self.current_size = snap_marker.current_size;
        self.current_snap -= 1;
        Ok(())
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        if self.current_snap == 0 {
            Err(FSError::Unknown)?
        }
        let snap_marker = self
            .snap_markers
            .last()
            .expect("Must exist since current_snap > 0");
        self.inodes.restore(snap_marker.meta_info_marker);
        self.data.restore(snap_marker.data_marker);
        self.current_size = snap_marker.current_size;
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
struct SnapMarker {
    data_marker: DataSnapMarker,
    meta_info_marker: MetaInfoSnapMarker,
    current_size: usize,
}
