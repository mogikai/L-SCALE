//! Provides an in memory file system implementation with snapshot capabilities for usage by fuzzers.
//! TODO: Implement and re-usage of deleted memory
//! TODO: Improve error handling with more meaningful errors
//! TODO: Implement update of stat struct.
use std::{fs, mem};
use std::cell::RefCell;
use std::cmp::{min, Ordering};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{Metadata, read};
use std::io;
use std::os::linux::fs::MetadataExt;
use std::path::{Path, PathBuf};

use byteorder::{ByteOrder, LittleEndian};

use crate::emulator::abstractions::handles::FSError;
use crate::emulator::system::filesystem::FSEntryContent::{Dir, File};

pub type FSEntryId = usize;
type ContentId = usize;
type SnapId = usize;

#[derive(Clone, Debug, Default)]
pub struct Stat {
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

impl From<Stat> for Vec<u8> {
    fn from(stat: Stat) -> Self {
        (&stat).into()
    }
}

impl From<&Stat> for Vec<u8> {
    fn from(stat: &Stat) -> Self {
        let mut v: Vec<u8> = vec![0; mem::size_of::<Stat>()];
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

#[derive(Debug, Copy, Clone)]
struct SnapMarker {
    data_marker: DataSnapMarker,
    meta_info_marker: MetaInfoSnapMarker,
    current_size: usize,
}

#[derive(Debug, Copy, Clone)]
struct DataSnapMarker {
    data_marker: usize,
}

#[derive(Debug, Copy, Clone)]
struct MetaInfoSnapMarker {
    content_marker: usize,
    next_fse_id: FSEntryId,
}

// TODO: Consider memory mapping of files into ram

#[derive(Debug)]
struct MetaInfo {
    content: Vec<FSEntry>,
    fse_ids: RefCell<Vec<HashMap<FSEntryId, Index>>>,
    next_fse_id: FSEntryId,
    current_snap: usize,
    root_node: FSEntryId,
}

#[derive(Debug)]
struct Data {
    data: Vec<u8>,
    free_blocks: Vec<BTreeMap<usize, Vec<Block>>>,
    current_snap: usize,
}

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

#[derive(Copy, Clone, Debug)]
struct HistoricalIndex {
    content_id: ContentId,
    snap_id: SnapId,
}

#[derive(Copy, Clone, Debug)]
enum Index {
    Historical(HistoricalIndex),
    Deleted,
}

impl Index {
    fn new(content_id: ContentId, snap_id: SnapId) -> Self {
        Self::Historical(
            HistoricalIndex {
                content_id,
                snap_id,
            }
        )
    }

    fn historical_or<E>(self, error: E) -> Result<HistoricalIndex, E> {
        match self {
            Index::Historical(historical_index) => Ok(historical_index),
            Index::Deleted => Err(error)
        }
    }
}

#[derive(Eq, Copy, Clone, Debug)]
struct Block {
    start: usize,
    size: usize,
    data_offset: usize,
    snap_id: SnapId,
}

#[derive(Clone, Debug)]
enum FSEntryContent {
    File { contents: BTreeSet<Block>, size: usize },
    // Block ids
    Dir { contents: BTreeMap<String, FSEntryId> },
}

#[derive(Clone, Debug)]
struct FSEntry {
    contents: FSEntryContent,
    stat: Stat,
    name: String,
}

impl FSEntry {
    fn new(contents: FSEntryContent, stat: Stat, name: String) -> Self {
        FSEntry {
            contents,
            stat,
            name,
        }
    }
}

impl MetaInfo {
    fn new() -> Self {
        let mut meta_info = MetaInfo {
            content: Vec::new(),
            fse_ids: RefCell::new(Vec::new()),
            next_fse_id: 1,
            current_snap: 0,
            root_node: 0,
        };
        meta_info.fse_ids.borrow_mut().push(HashMap::new());
        let root_stat = Stat::default();
        meta_info.add_entry(0, FSEntry::new(Dir { contents: BTreeMap::new() }, root_stat, "/".into())).unwrap();
        meta_info
    }

    /// Creates a new FSEntry in the file system.
    ///
    /// The new FSEntry is obtained by a call to the supplied closure new_entry
    /// and is subsequently inserted into the file systems structures.
    /// If The prefix of the path does not exist, or if the last element
    /// of the path already exists, an error is returned.
    fn new_entry(&mut self, path: &Path, new_entry: impl FnOnce(String) -> FSEntry) -> Result<FSEntryId, FSError> {
        let dir_entry_id = self.resolve_last(path.parent().ok_or(FSError::Unknown)?)?;
        let dir_content_id = self.most_recent_content_id(dir_entry_id)?;
        if let Dir { contents } = &self.get_content_by_id(dir_content_id)?.contents {
            let name = path.file_name().ok_or(FSError::Unknown)?.to_str().ok_or(FSError::Unknown)?;
            if contents.contains_key(name) {
                return Err(FSError::Unknown);
            }
            let entry = new_entry(name.into());
            let entry_id = self.next_fse_id;
            self.next_fse_id += 1;
            self.add_entry(entry_id, entry)?;
            let dir_entry = self.most_recent_entry_mut(dir_entry_id)
                .expect("The entry exists at this point, since we already accessed it");
            if let Dir { contents } = &mut dir_entry.contents {
                contents.insert(name.into(), entry_id);
                Ok(entry_id)
            } else {
                panic!("Should never happen");
            }
        } else {
            Err(FSError::Unknown)
        }
    }

    /// See the public api [Filesystem::create_file](Filesystem::create_file) for documentation.
    fn create_file(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.new_entry(path, |name| FSEntry::new(File { contents: BTreeSet::new(), size: 0 }, stat.unwrap_or(Stat::default()), name))
    }

    /// See the public api [Filesystem::create_dir](Filesystem::create_dir) for documentation.
    fn create_dir(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.new_entry(path, |name| FSEntry::new(Dir { contents: BTreeMap::new() }, stat.unwrap_or(Stat::default()), name))
    }

    /// See the public api [Filesystem::delete](Filesystem::delete) for documentation.
    fn delete(&mut self, path: &Path) -> Result<(), FSError> {
        let fse_ids = self.resolve_path(path)?;


        let (entry, snap_id) = self.most_recent_entry_and_snap_id(*fse_ids.last().expect("Will always contain at least one id"))?;
        match &entry.contents {
            Dir { contents } => if !contents.is_empty() { return Err(FSError::Unknown); },
            File { contents, .. } => {
                if snap_id == self.current_snap {
                    // contents.drain_filter(|b| b.snap_id == self.current_snap);
                    // TODO: take out blocks that are from current snapshot and add them to free set
                }
            }
        }


        // Remove the historical index for the current snapshot if it exists.
        // This is ok, since we do not have to be able to reset this change.
        // Old snapshots must not be touched.
        // The entry itself will persist
        // Instead of removing however, we insert a deleted index marker.
        // This is needed to avoid going backwards in the snapshots and
        // finding one where the file still exists.
        self.fse_ids.borrow_mut()[self.current_snap].insert(*fse_ids.last().unwrap(), Index::Deleted);

        // Remove entry from parent dir
        let dir = self.most_recent_entry_mut(fse_ids.len().checked_sub(2).map(|i| fse_ids[i]).ok_or(FSError::Unknown)?)?;
        if let Dir { contents } = &mut dir.contents {
            let name = path.file_name().expect("The path resolved, so the name has to exist").to_str().unwrap();
            contents.remove(name);
        }
        Ok(())
    }

    /// Returns the most recent entry mutably. If the entry is historical, a new copy is created and inserted,
    /// such that the entry may be modified in the context of the current snapshot.
    ///
    fn most_recent_entry_mut(&mut self, fse_id: FSEntryId) -> Result<&mut FSEntry, FSError> {
        let historical_index = self.most_recent_historical_index(fse_id)?.historical_or(FSError::Unknown)?;
        if historical_index.snap_id < self.current_snap {
            // Historical. Cannot modify. Need to clone first.
            let entry = self.content.get(historical_index.content_id).expect("id must be valid");
            let new_entry = entry.clone();
            self.add_entry(fse_id, new_entry)?;
            Ok(self.most_recent_entry_mut(fse_id)?)
        } else {
            Ok(self.content.get_mut(historical_index.content_id).expect("id must be valid"))
        }
    }

    fn add_entry(&mut self, fse_id: FSEntryId, entry: FSEntry) -> Result<(), FSError> {
        self.content.push(entry);
        let result = self.fse_ids.borrow_mut()[self.current_snap].insert(fse_id, Index::new(self.content.len() - 1, self.current_snap));
        if let Some(result) = result {
            if result.historical_or(FSError::Unknown)?.snap_id == self.current_snap {
                panic!("Index reused. This must never happen");
            }
        }
        Ok(())
    }


    /// See the public api [Filesystem::resolve_path](Filesystem::resolve_path) for documentation.
    fn resolve_path(&self, path: &Path) -> Result<Vec<FSEntryId>, FSError> {
        if !path.is_absolute() {
            return Err(FSError::Unknown);
        }
        let mut path_ids = Vec::new();
        path_ids.push(self.root_node);
        let path = path.strip_prefix("/").expect("Absolute path always has / as prefix");
        for component in path {
            if let Dir { contents } = &self.most_recent_entry(*path_ids.last().expect("Path always contains root"))?.contents {
                let child = *contents.get(component.to_str().unwrap()).ok_or(FSError::Unknown)?;
                path_ids.push(child);
            }
        }
        Ok(path_ids)
    }

    /// See the public api [Filesystem::resolve_last](Filesystem::resolve_last) for documentation.
    fn resolve_last(&self, path: &Path) -> Result<FSEntryId, FSError> {
        Ok(*self.resolve_path(path)?.last().expect("Empty paths cannot occur"))
    }

    fn most_recent_historical_index(&self, index: FSEntryId) -> Result<Index, FSError> {
        let mut fse_ids = self.fse_ids.borrow_mut();
        let mut historical_index = fse_ids[self.current_snap].get(&index).cloned();
        if historical_index.is_none() {
            for i in (0..self.current_snap).rev() {
                historical_index = fse_ids[i].get(&index).cloned();
                if let Some(historical_index) = historical_index {
                    // Cache for next lookup.
                    fse_ids[self.current_snap].insert(index, historical_index);
                    break;
                }
            }
        };
        historical_index.ok_or(FSError::Unknown)
    }

    fn most_recent_snap(&self, index: FSEntryId) -> Result<SnapId, FSError> {
        Ok(self.most_recent_historical_index(index)?.historical_or(FSError::Unknown)?.snap_id)
    }

    fn most_recent_content_id(&self, index: FSEntryId) -> Result<ContentId, FSError> {
        Ok(self.most_recent_historical_index(index)?.historical_or(FSError::Unknown)?.content_id)
    }

    fn most_recent_entry(&self, index: FSEntryId) -> Result<&FSEntry, FSError> {
        Ok(self.content.get(self.most_recent_content_id(index)?).expect("The id must be valid"))
    }

    fn most_recent_entry_and_snap_id(&self, index: FSEntryId) -> Result<(&FSEntry, SnapId), FSError> {
        let historical_index = self.most_recent_historical_index(index)?.historical_or(FSError::Unknown)?;
        Ok((self.content.get(historical_index.content_id).expect("The id must be valid"), historical_index.snap_id))
    }

    fn get_content_by_id(&self, content_id: ContentId) -> Result<&FSEntry, FSError> {
        Ok(self.content.get(content_id).ok_or(FSError::Unknown)?)
    }

    fn print_tree(&self, root: FSEntryId, depth: Option<usize>) {
        let depth = depth.unwrap_or(0);
        let node = self.most_recent_entry(root).unwrap();
        println!("{} {}", vec![' '; depth * 3].into_iter().chain("|-".chars()).collect::<String>(), node.name);
        match &node.contents {
            File { .. } => {}
            Dir { contents } => {
                for entry in contents.values() {
                    self.print_tree(*entry, Some(depth + 1));
                }
            }
        }
    }

    fn _restore(&mut self, snap_marker: MetaInfoSnapMarker) {
        self.content.truncate(snap_marker.content_marker);
        let mut fse_ids = self.fse_ids.borrow_mut();
        fse_ids.truncate(self.current_snap);
        fse_ids.push(HashMap::new());
        self.next_fse_id = snap_marker.next_fse_id;
    }

    fn snapshot(&mut self) -> MetaInfoSnapMarker {
        self.fse_ids.borrow_mut().push(HashMap::new());
        MetaInfoSnapMarker {
            content_marker: self.content.len(),
            next_fse_id: self.next_fse_id,
        }
    }
}

impl Data {
    fn write_to_block_cow(&mut self, pos: usize, data: &[u8], contents: &mut BTreeSet<Block>, block_start: usize) -> Result<usize, FSError> {
        let block = contents.take(&block_start).expect("The block_starts were constructed beforehand and must exist");
        // We can now split the block if COW is necessary, or modify directly and reinsert
        let block = if block.snap_id < self.current_snap {
            assert!(block.contains(pos));
            let end = if block.contains(pos + data.len()) {
                pos + data.len()
            } else {
                block.end()
            };
            let (front, mut center, back) = block.split(align_4k_down(pos), align_4k(end))?;
            front.map(|b| contents.insert(b));
            back.map(|b| contents.insert(b));
            let new_data_offset = self.data.len();
            self.data.extend_from_within(block.data_offset..block.data_offset + block.size);
            center.data_offset = new_data_offset;
            center
        } else {
            block
        };

        assert!(block.contains(pos));
        let block_offset = block.data_offset + (pos - block.start);
        let amount = min(block.end() - pos, data.len());
        self.data[block_offset..block_offset + amount]
            .copy_from_slice(&data[..amount]);

        contents.insert(block);
        Ok(amount)
    }

    /// Inserts a new block at the end of the existing blocks and writes data into the block.
    /// If pos is after the end of the last block, the area in between will be allocated for
    /// this block and filled with zeros. This guarantees that blocks are always continuous.
    fn new_block(&mut self, pos: usize, contents: &mut BTreeSet<Block>, data: &[u8]) -> Result<(), FSError> {
        // TODO: First search for unused existing blocks
        let last_block_end = contents.iter().last().map_or(0, |b| b.end());
        assert!(pos >= last_block_end);
        let fill = pos - last_block_end;
        let alignment = align_4k(data.len() + fill) - data.len() - fill;
        let block = Block::new(last_block_end, data.len() + fill + alignment, self.data.len(), self.current_snap)?;
        self.data.resize(self.data.len() + fill, 0);
        self.data.extend_from_slice(&data);
        self.data.resize(self.data.len() + alignment, 0);
        contents.insert(block);
        Ok(())
    }

    /// Attempts to read as much data as possible from the supplied block at position pos
    ///
    fn read_block(&self, pos: usize, block: &Block, buffer: &mut [u8]) -> Result<usize, FSError> {
        if !block.contains(pos) {
            return Err(FSError::Unknown);
        }
        let to_read = min(buffer.len(), block.end() - pos);
        let offset = block.data_offset + (pos - block.start);
        buffer[..to_read].copy_from_slice(&self.data[offset..offset + to_read]);
        Ok(to_read)
    }

    fn save_free_blocks(&mut self, blocks: impl ExactSizeIterator<Item=Block>) {
        self.free_blocks.reserve(blocks.len());
        blocks.for_each(|b| {
            // We only want to save blocks that are used in the current snapshot.
            // Old blocks are copied anyways on write and need to be preserved.
            if b.snap_id == self.current_snap {
                self.free_blocks[self.current_snap].entry(b.size).or_insert(Vec::new()).push(b);
            }
        });
    }

    fn _restore(&mut self, snap_marker: DataSnapMarker) {
        self.free_blocks.truncate(self.current_snap);
        self.free_blocks.push(BTreeMap::new());
        self.data.truncate(snap_marker.data_marker);
    }

    fn snapshot(&mut self) -> DataSnapMarker {
        self.free_blocks.push(BTreeMap::new());
        DataSnapMarker {
            data_marker: self.data.len(),
        }
    }
}

impl Filesystem {
    pub fn new(size: usize) -> Self {
        Filesystem {
            on_disk_root: None,
            data: Data {
                data: Vec::new(),
                free_blocks: vec![BTreeMap::new()],
                current_snap: 0,
            },
            snap_markers: Vec::new(),
            meta_info: MetaInfo::new(),
            current_size: 0,
            max_size: size,
            current_snap: 0,
        }
    }

    pub fn from_disk(size: usize, root: &Path) -> Result<Self, FSError> {
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

    pub fn read(&self, file_id: FSEntryId, pos: usize, len: usize) -> Result<Vec<u8>, FSError> {
        if let File { contents, size } = &self.meta_info.most_recent_entry(file_id)?.contents {
            if pos > *size {
                return Err(FSError::Unknown);
            }
            let len = min(len, size - pos);
            let mut read = 0;
            let start_block = contents.range(..=pos).last().expect("Must exist since pos < size");
            let mut buffer = Vec::with_capacity(len);
            unsafe { buffer.set_len(len); }
            for block in contents.range(start_block.start..pos + len) {
                read += self.data.read_block(pos + read, block, &mut buffer[read..])?;
            }
            Ok(buffer)
        } else {
            Err(FSError::Unknown)
        }
    }

    pub fn write(&mut self, file_id: FSEntryId, pos: usize, data: &[u8]) -> Result<usize, FSError> {
        if let File { contents, size } = &mut self.meta_info.most_recent_entry_mut(file_id)?.contents {
            if (pos + data.len()).checked_sub(*size).unwrap_or(0) > self.max_size - self.current_size {
                // No space available
                println!("NO SPACE! remaining: {}, want: {}", self.max_size - self.current_size, (pos + data.len()).checked_sub(*size).unwrap_or(0));
                return Err(FSError::Unknown);
            }

            let start_block = contents.range(..=pos).last();
            let mut written = 0;
            if let Some(start_block) = start_block {
                if start_block.contains(pos) {
                    for block_start
                    in contents.range(start_block.start..pos + data.len())
                        .map(|b| b.start)
                        .collect::<Vec<_>>() {
                        written += self.data.write_to_block_cow(pos + written, &data[written..], contents, block_start)?;
                    }
                }
            }

            // Write the remaining data to a new block.
            if data.len() > written {
                self.data.new_block(pos + written, contents, &data[written..])?;
                written = data.len();
            }

            let additional_size = (pos + written).checked_sub(*size).unwrap_or(0);
            *size += additional_size;
            self.current_size += additional_size;

            Ok(written)
        } else {
            Err(FSError::Unknown)
        }
    }

    pub fn truncate(&mut self, file_id: FSEntryId, new_size: usize) -> Result<(), FSError> {
        let size = if let File { size, .. } = &self.meta_info.most_recent_entry(file_id)?.contents {
            Ok(*size)
        } else {
            Err(FSError::Unknown)
        }?;

        if new_size > size {
            let buf = vec![0; new_size - size];
            self.write(file_id, new_size - size, &buf)?;
            Ok(())
        } else if let File { contents, size } = &mut self.meta_info.most_recent_entry_mut(file_id)?.contents {
            self.current_size -= *size - new_size;
            *size = new_size;
            self.data.save_free_blocks(contents.split_off(&new_size).into_iter());
            if let Some(block) = contents.iter().last().cloned() {
                self.data.write_to_block_cow(new_size, &vec![0; block.end() - new_size].as_slice(), contents, block.start)?;
            }
            Ok(())
        } else {
            Err(FSError::Unknown)
        }
    }

    pub fn create_file(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.meta_info.create_file(path, stat)
    }

    pub fn create_file_with_contents(&mut self, path: &Path, data: &[u8], stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        let fse_id = self.create_file(path, stat)?;
        self.write(fse_id, 0, data)?;
        Ok(fse_id)
    }

    pub fn create_dir(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.meta_info.create_dir(path, stat)
    }

    pub fn delete(&mut self, path: &Path) -> Result<(), FSError> {
        self.meta_info.delete(path)
    }

    pub fn stat(&self, file_id: FSEntryId) -> Result<&Stat, FSError> {
        Ok(&self.meta_info.most_recent_entry(file_id)?.stat)
    }

    /// This function resolves the supplied path to a list of FSEntryId. After the lookup,
    /// the file or dirs in the path can be directly referenced by using the returned FSEntryIds.
    /// The FSEntryIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FSEntryId of each element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_path(&self, path: &Path) -> Result<Vec<FSEntryId>, FSError> {
        self.meta_info.resolve_path(path)
    }

    /// This function resolves the supplied path to a list of FSEntryId. After the lookup,
    /// the file or dirs in the path can be directly referenced by using the returned FSEntryIds.
    /// The FSEntryIds are valid for as long as the elements exist and will result in an error
    /// if the referenced element was deleted in the meantime.
    ///
    /// returns: The FSEntryId of each element in the path if it resolves. Otherwise an Error is returned.
    pub fn resolve_last(&self, path: &Path) -> Result<FSEntryId, FSError> {
        self.meta_info.resolve_last(path)
    }

    fn print_tree(&self) {
        self.meta_info.print_tree(self.meta_info.root_node, None);
    }

    pub fn restore_and_discard(&mut self) -> Result<(), FSError> {
        if self.current_snap == 0 {
            return Err(FSError::Unknown);
        }
        let snap_marker = self.snap_markers.pop().expect("Must exist since current_snap > 0");
        self._restore(snap_marker);
        self.current_snap -= 1;
        self.meta_info.current_snap -= 1;
        self.data.current_snap -= 1;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<(), FSError> {
        if self.current_snap == 0 {
            return Err(FSError::Unknown);
        }
        let snap_marker = self.snap_markers.last().expect("Must exist since current_snap > 0");
        self._restore(*snap_marker);
        Ok(())
    }

    fn _restore(&mut self, snap_marker: SnapMarker) {
        self.data._restore(snap_marker.data_marker);
        self.meta_info._restore(snap_marker.meta_info_marker);
        self.current_size = snap_marker.current_size;
    }

    pub fn snapshot(&mut self) {
        assert_eq!(self.snap_markers.len(), self.current_snap);
        self.snap_markers.push(SnapMarker {
            data_marker: self.data.snapshot(),
            meta_info_marker: self.meta_info.snapshot(),
            current_size: self.current_size,
        });
        self.current_snap += 1;
        self.meta_info.current_snap = self.current_snap;
        self.data.current_snap = self.current_snap;
    }
}

fn load_dir(fs: &mut Filesystem, dir: &Path, root: &Path) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let stat = entry.metadata().unwrap();
            if path.is_dir() {
                fs.create_dir(&*Path::new("/").join(path.strip_prefix(root).unwrap().to_path_buf()), Some(stat.into())).unwrap();
                load_dir(fs, &path, root)?;
                // FSEntry::DirEntry(Dir {}, Stats::from(stats))
            } else {
                let data = read(entry.path())?;
                fs.create_file_with_contents(&*Path::new("/").join(path.strip_prefix(root).unwrap().to_path_buf()), data.as_slice(), Some(stat.into())).unwrap();
                // FSEntry::FileEntry(File::new(&path), Stats::from(stats))
            };
            // println!("{:?}", Path::new("/").join(path.strip_prefix(root).unwrap().to_path_buf()));
        }
    }
    Ok(())
}

impl Block {
    fn new(start: usize, size: usize, data_offset: usize, snap_id: SnapId) -> Result<Self, FSError> {
        if !is_4k_aligned(start) || !is_4k_aligned(size) || !is_4k_aligned(data_offset) {
            return Err(FSError::Unknown);
        }
        Ok(Self {
            start,
            size,
            data_offset,
            snap_id,
        })
    }

    fn end(&self) -> usize {
        self.start + self.size
    }

    fn contains(&self, pos: usize) -> bool {
        self.start <= pos && pos <= self.start + self.size
    }

    fn has_space(&self, pos: usize, size: usize) -> bool {
        self.contains(pos) && self.contains(pos + size)
    }

    fn split(self, start: usize, end: usize) -> Result<(Option<Block>, Block, Option<Block>), FSError> {
        if !is_4k_aligned(start) || !is_4k_aligned(end) {
            return Err(FSError::Unknown);
        }
        assert!(self.start <= start);
        assert!(end <= self.end());
        let front_size = start - self.start;
        let back_size = self.end() - end;
        let front = if front_size > 0 {
            Some(Block::new(self.start, front_size, self.data_offset, self.snap_id)?)
        } else {
            None
        };

        let center = Block::new(
            self.start + front_size,
            self.size - front_size - back_size,
            self.data_offset + front_size,
            self.snap_id,
        )?;

        let back = if back_size > 0 {
            Some(Block::new(end, back_size, self.data_offset + front_size + center.size, self.snap_id)?)
        } else {
            None
        };
        Ok((front, center, back))
    }
}

const MASK4K: usize = 4 * (1 << 10) - 1;

fn is_4k_aligned(val: usize) -> bool { val & MASK4K == 0 }

fn align_4k(val: usize) -> usize { (val + MASK4K) & !MASK4K }

fn align_4k_down(val: usize) -> usize { val & !MASK4K }

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.start.eq(&other.start) || self.data_offset.eq(&other.data_offset)
    }
}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.start.partial_cmp(&other.start)
    }
}

impl Ord for Block {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl std::borrow::Borrow<usize> for Block {
    fn borrow(&self) -> &usize {
        &self.start
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use rand::{RngCore, thread_rng};

    use crate::emulator::system::filesystem::Filesystem;

    #[test]
    fn basic() {
        let mut fs = Filesystem::new(2_621_440);
        assert!(fs.create_file(Path::new("/my_file.txt"), None).is_ok());
        assert!(fs.create_file(Path::new("/my_file.txt"), None).is_err());
        assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_ok());
        println!("{:?}", fs);
        assert!(fs.restore().is_err());
        println!("Before Snapshot: ");
        fs.print_tree();
        fs.snapshot();

        assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_err());
        assert!(fs.delete(Path::new("/my_file2.txt")).is_ok());
        assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_ok());
        assert!(fs.delete(Path::new("/my_file2.txt")).is_ok());
        assert!(fs.create_dir(Path::new("/bin"), None).is_ok());
        assert!(fs.create_dir(Path::new("/home"), None).is_ok());
        assert!(fs.create_dir(Path::new("/home/mark"), None).is_ok());
        assert!(fs.delete(Path::new("/home")).is_err());
        assert!(fs.create_dir(Path::new("/home/mark/my_stuff"), None).is_ok());
        assert!(fs.create_file(Path::new("/home/mark/my_stuff/my_file.txt"), None).is_ok());
        println!("Before Snapshot: ");
        fs.print_tree();
        fs.snapshot();

        assert!(fs.create_dir(Path::new("/bin/test"), None).is_ok());
        assert!(fs.create_file(Path::new("/bin/test/my_file.bin"), None).is_ok());
        assert!(fs.create_dir(Path::new("/my_file.txt/test"), None).is_err());
        assert!(fs.create_dir(Path::new("/bin/test"), None).is_err());
        assert!(fs.create_dir(Path::new("/doesnotexist/test"), None).is_err());

        println!("Before restore: ");
        fs.print_tree();
        assert!(fs.restore_and_discard().is_ok());
        println!("After restore: ");
        fs.print_tree();
        let resolved = fs.resolve_path(Path::new("/home/mark/my_stuff/my_file.txt"));
        assert!(resolved.is_ok());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.len(), 5);
        println!("After restore: ");
        assert!(fs.restore().is_ok());
        fs.print_tree();
        for i in 0..10_000 {
            if i % 1000 == 0 {
                fs.snapshot();
            }
            assert!(fs.create_file(Path::new(&format!("/test{}.txt", i)), None).is_ok())
        }
        for i in 0..10_000 {
            if i % 1000 == 0 {
                fs.restore_and_discard().unwrap();
            }
        }
        assert!(fs.restore().is_ok());
        fs.print_tree();
        println!("{:?}", fs);
        let my_file = fs.resolve_last(Path::new("/my_file.txt")).unwrap();
        assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_ok());
        assert!(fs.delete(Path::new("/my_file.txt")).is_ok());
        assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_err());
    }

    #[test]
    fn load_file_contents() {
        let mut fs = Filesystem::new(1_073_741_824);
        let data = "This a sample text file\n It should contain two lines and a trailing newline\n".as_bytes();
        let data2 = "This a different sample text file\n It should contain two lines and a trailing newline and some fancy shit WOOOOO\n".as_bytes();
        let result = fs.create_file_with_contents(Path::new("/test.txt"), data, None);
        assert!(result.is_ok());
        let result2 = fs.create_file_with_contents(Path::new("/test2.txt"), data2, None);
        assert!(result2.is_ok());
        let file_id = result.unwrap();
        let file_id2 = result2.unwrap();
        assert_eq!(data, fs.read(file_id, 0, data.len()).unwrap().as_slice());
        assert_eq!(data2, fs.read(file_id2, 0, data2.len()).unwrap().as_slice());
        assert!(fs.write(file_id, 10000, data).is_ok());
        assert!(fs.write(file_id2, 10000, data).is_ok());
        assert!(fs.write(file_id, 10010, data).is_ok());
        fs.snapshot();
        fs.write(file_id, 5000, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_bytes()).unwrap();
        fs.restore_and_discard().unwrap();
        fs.snapshot();
        for i in 1..1_000 {
            let mut rdata = [0u8; 10000];
            thread_rng().fill_bytes(&mut rdata);
            fs.write(file_id, i * 10, &rdata).unwrap();
            fs.read(file_id, (i - 1) * 10, 10000).unwrap();
            if i % 1_000 == 0 {
                fs.restore().unwrap();
            }
        }
        fs.restore().unwrap();

        let data3 = "Beep Boop".as_bytes();
        let result3 = fs.create_file_with_contents(Path::new("/test3.txt"), data3, None);
        assert!(result3.is_ok());
        let file_id3 = result3.unwrap();
        assert_eq!(data3, fs.read(file_id3, 0, data3.len()).unwrap());
        println!("{:?}", fs.read(file_id3, 0, 10000));
        fs.truncate(file_id3, 5000).unwrap();
        println!("{:?}", fs.read(file_id3, 0, 10000));
        fs.snapshot();
        fs.truncate(file_id3, 4).unwrap();
        println!("{:?}", fs.read(file_id3, 0, 10000));
        fs.truncate(file_id3, 5000).unwrap();
        println!("{:?}", fs.read(file_id3, 0, 10000));
        fs.restore().unwrap();
        fs.truncate(file_id3, 10).unwrap();
        println!("{:?}", fs.read(file_id3, 0, 10000));
    }

    #[test]
    fn load_fs() {
        let mut fs = Filesystem::from_disk(1_073_741_824, Path::new("/home/mark/projects/fraunhofer/fuzzing/prototypes/ffs")).unwrap();
        // fs.print_tree();
        let lib_id = fs.resolve_last(Path::new("/src/lib.rs")).unwrap();
        let data = fs.read(lib_id, 0, 100000).unwrap();
        println!("{}", String::from_utf8(data).unwrap());
    }

    #[test]
    fn size_after_reset_is_correct() {
        let mut fs = Filesystem::new(1000);
        let test_file = Path::new("/test.txt");
        fs.create_file(test_file, None).unwrap();
        let test_file_id = fs.resolve_last(test_file).unwrap();
    }
}
