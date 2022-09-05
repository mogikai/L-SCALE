use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::emulator::abstractions::handles::FSError;
use crate::emulator::system::filesystem::{ContentId, FSEntryId, SnapId};
use crate::emulator::system::filesystem::block::Block;
use crate::emulator::system::filesystem::stat::Stat;

use self::FSEntryContent::{Dir, File};

#[derive(Debug)]
pub struct MetaInfo {
    content: Vec<FSEntry>,
    fse_ids: RefCell<Vec<HashMap<FSEntryId, Index>>>,
    next_fse_id: FSEntryId,
    current_snap: usize,
    root_node: FSEntryId,
}

impl MetaInfo {
    pub(super) fn new() -> Self {
        let mut meta_info = Self {
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
    pub(super) fn new_entry(&mut self, path: &Path, new_entry: impl FnOnce(String) -> FSEntry) -> Result<FSEntryId, FSError> {
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
    pub(super) fn create_file(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.new_entry(path, |name| FSEntry::new(File { contents: BTreeSet::new(), size: 0 }, stat.unwrap_or_default(), name))
    }

    /// See the public api [Filesystem::create_dir](Filesystem::create_dir) for documentation.
    pub(super) fn create_dir(&mut self, path: &Path, stat: Option<Stat>) -> Result<FSEntryId, FSError> {
        self.new_entry(path, |name| FSEntry::new(Dir { contents: BTreeMap::new() }, stat.unwrap_or_default(), name))
    }

    /// See the public api [Filesystem::delete](Filesystem::delete) for documentation.
    pub(super) fn delete(&mut self, path: &Path) -> Result<(), FSError> {
        let fse_ids = self.resolve_path(path)?;


        let (entry, snap_id) = self.most_recent_entry_and_snap_id(*fse_ids.last().expect("Will always contain at least one id"))?;
        match &entry.contents {
            Dir { contents } => if !contents.is_empty() { return Err(FSError::Unknown); },
            File { .. } => {
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

    pub(super) fn add_entry(&mut self, fse_id: FSEntryId, entry: FSEntry) -> Result<(), FSError> {
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
    pub(super) fn resolve_path(&self, path: &Path) -> Result<Vec<FSEntryId>, FSError> {
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
    pub(super) fn resolve_last(&self, path: &Path) -> Result<FSEntryId, FSError> {
        Ok(*self.resolve_path(path)?.last().expect("Empty paths cannot occur"))
    }

    pub(super) fn root(&self) -> FSEntryId {
        self.root_node
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

    /// Returns the most recent entry mutably. If the entry is historical, a new copy is created and inserted,
    /// such that the entry may be modified in the context of the current snapshot.
    ///
    pub(super) fn most_recent_entry_mut(&mut self, fse_id: FSEntryId) -> Result<&mut FSEntry, FSError> {
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

    pub(super) fn most_recent_entry(&self, index: FSEntryId) -> Result<&FSEntry, FSError> {
        Ok(self.content.get(self.most_recent_content_id(index)?).expect("The id must be valid"))
    }

    fn most_recent_entry_and_snap_id(&self, index: FSEntryId) -> Result<(&FSEntry, SnapId), FSError> {
        let historical_index = self.most_recent_historical_index(index)?.historical_or(FSError::Unknown)?;
        Ok((self.content.get(historical_index.content_id).expect("The id must be valid"), historical_index.snap_id))
    }

    fn get_content_by_id(&self, content_id: ContentId) -> Result<&FSEntry, FSError> {
        self.content.get(content_id).ok_or(FSError::Unknown)
    }

    pub(super) fn print_tree(&self, root: FSEntryId, depth: Option<usize>) {
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

    pub(super) fn restore_and_discard(&mut self, snap_marker: MetaInfoSnapMarker) {
        self.restore(snap_marker);
        self.current_snap -= 1;
    }

    pub(super) fn restore(&mut self, snap_marker: MetaInfoSnapMarker) {
        self.content.truncate(snap_marker.content_marker);
        let mut fse_ids = self.fse_ids.borrow_mut();
        fse_ids.truncate(self.current_snap);
        fse_ids.push(HashMap::new());
        self.next_fse_id = snap_marker.next_fse_id;
    }

    pub(super) fn snapshot(&mut self) -> MetaInfoSnapMarker {
        self.fse_ids.borrow_mut().push(HashMap::new());
        self.current_snap += 1;
        MetaInfoSnapMarker {
            content_marker: self.content.len(),
            next_fse_id: self.next_fse_id,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum FSEntryContent {
    File { contents: BTreeSet<Block>, size: usize },
    // Block ids
    Dir { contents: BTreeMap<String, FSEntryId> },
}

#[derive(Clone, Debug)]
pub(super) struct FSEntry {
    pub(super) contents: FSEntryContent,
    pub(super) stat: Stat,
    pub(super) name: String,
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

#[derive(Debug, Copy, Clone)]
pub struct MetaInfoSnapMarker {
    content_marker: usize,
    next_fse_id: FSEntryId,
}
