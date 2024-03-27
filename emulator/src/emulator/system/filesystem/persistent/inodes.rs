use crate::emulator::abstractions::filesystem::{
    FSEntry,
    FSEntryId,
};
use libc::wchar_t;
use std::cell::RefCell;
use std::collections::{
    BTreeMap,
    BTreeSet,
    HashMap,
};
use std::path::{
    Path,
    PathBuf,
};
use tracing::trace;

use crate::emulator::system::filesystem::stat::Stat;
use crate::emulator::system::filesystem::{
    ContentId,
    SnapId,
};

use mogi_system::errors::filesystem::FSError;
use std::ffi::OsString;

use crate::emulator::system::filesystem::persistent::block::Block;

#[derive(Debug)]
pub(super) struct Inodes {
    content: Vec<Inode>,
    fse_ids: RefCell<Vec<HashMap<FSEntryId, Index>>>,
    next_fse_id: FSEntryId,
    current_snap: usize,
}

impl Inodes {
    pub(super) fn new() -> Self {
        Self {
            content: Vec::new(),
            fse_ids: RefCell::new(vec![HashMap::new()]),
            next_fse_id: 1.into(),
            current_snap: 0,
        }
    }

    /// Creates a new FSEntry in the file system.
    ///
    /// The new FSEntry is obtained by a call to the supplied closure new_entry
    /// and is subsequently inserted into the file systems structures.
    /// If The prefix of the path does not exist, or if the last element
    /// of the path already exists, an error is returned.
    pub(super) fn new_entry(
        &mut self,
        parent: FSEntryId,
        name: OsString,
        entry: FSEntry,
        stat: Option<Stat>,
    ) -> Result<FSEntryId, FSError> {
        trace!("new entry in {:?} name {:?}", parent, name);
        if name == "." || name == ".." {
            return Err(FSError::Invalid);
        }
        let entry_id = self.next_fse_id;
        let mut p = self.most_recent_entry_mut(parent)?;
        if p.file_type != FSEntry::Dir {
            return Err(FSError::NoEntry);
        }
        if let Some(FSContent::Dir { ref mut entries }) = p.content {
            let content = match entry {
                FSEntry::Dir => Some(FSContent::Dir {
                    entries: BTreeMap::new(),
                }),
                FSEntry::File => Some(FSContent::File {
                    blocks: Default::default(),
                }),
                _ => None,
            };
            let e: Inode = Inode::new(entry, stat.unwrap_or_default(), content);
            let err = entries.try_insert(name, entry_id);
            trace!("try insert: {:?}", err);
            err.ok().ok_or(FSError::Exist)?;
            self.add_entry(entry_id, e);
            self.next_fse_id += 1;
            return Ok(entry_id);
        }
        Err(FSError::Unknown)
    }

    /// See the public api [Filesystem::delete](Filesystem::delete) for documentation.
    /// TODO check if change argument to fsentryid
    pub(super) fn delete(
        &mut self,
        dir_id: FSEntryId,
        name: OsString,
    ) -> Result<(), FSError> {
        // TODO: Handle deleting relative paths.
        trace!("delete at {:?} name: {:?}", dir_id, name);
        let dir = self.most_recent_entry(dir_id)?;
        if dir.file_type != FSEntry::Dir {
            return Err(FSError::NoEntry);
        }
        let entry_id = match &dir.content {
            Some(FSContent::Dir { entries }) => *entries.get(&name).ok_or(FSError::NoEntry)?,
            _ => {
                return Err(FSError::Unknown);
            }
        };

        let (entry, snap_id) = self.most_recent_entry_and_snap_id(entry_id)?;
        if let FSEntry::Mount { .. } = entry.file_type {
            return Err(FSError::Busy);
        }
        match &entry.content {
            Some(FSContent::Dir { entries }) => {
                if !entries.is_empty() {
                    return Err(FSError::NotEmpty);
                }
            }
            Some(FSContent::File { .. }) => {
                if snap_id == self.current_snap {
                    // contents.drain_filter(|b| b.snap_id == self.current_snap);
                    //TODO: take out blocks that are from current snapshot and add them to free set
                }
            }
            None => {}
        }
        self.fse_ids.borrow_mut()[self.current_snap].insert(entry_id, Index::Deleted);
        let mut dir = self.most_recent_entry_mut(dir_id)?;
        match &mut dir.content {
            Some(FSContent::Dir { ref mut entries }) => {
                trace!("remove direntry {:?} from {:?}", name, dir_id);
                entries.remove(&name).ok_or(FSError::Unknown)?;
                trace!("after remove: content {:?}", self.content);
                trace!("historical index: {:?}", self.fse_ids.borrow_mut()[self.current_snap]);
            }
            _ => {
                return Err(FSError::Unknown);
            }
        }
        Ok(())
    }

    pub(super) fn add_entry(
        &mut self,
        fse_id: FSEntryId,
        entry: Inode,
    ) -> Result<(), FSError> {
        self.content.push(entry);
        trace!("add entry: content: {:?}", self.content);
        let result = self.fse_ids.borrow_mut()[self.current_snap]
            .insert(fse_id, Index::new(self.content.len() - 1, self.current_snap));
        if let Some(result) = result {
            if result.historical_or(FSError::Unknown)?.snap_id == self.current_snap {
                panic!("Index reused. This must never happen");
            }
        }
        Ok(())
    }

    pub(super) fn path_from(
        &self,
        fse_id: FSEntryId,
    ) -> Result<PathBuf, FSError> {
        todo!();
        /*
        let mut path = vec![];
        let mut current_id = fse_id;

        while current_id != self.root_node {
            let entry = self.most_recent_entry(current_id)?;
            path.push(entry.name.clone());
            current_id = entry.parent;
        }

        path.push(String::from("/"));

        Ok(path.into_iter().rev().collect())
         */
    }

    /// See the public api [Filesystem::resolve_path](Filesystem::resolve_path) for documentation.
    pub(super) fn resolve_path(
        &self,
        root: FSEntryId,
        path: &Path,
    ) -> Result<(Vec<FSEntryId>, Option<PathBuf>), FSError> {
        trace!("meta_info: resolve_path: {:?} {:?}", self.most_recent_entry(root), path);
        let path = if path.is_absolute() {
            path.strip_prefix("/")
                .expect("Absolute path always has / as prefix")
        } else {
            path
        };

        let mut path_ids = vec![root];
        let mut remainder = Option::<PathBuf>::None;
        for component in path {
            if let Some(remainder) = &mut remainder {
                remainder.push(component);
                continue;
            }
            match &self
                .most_recent_entry(*path_ids.last().expect("Path always contains root"))?
                .content
            {
                Some(FSContent::Dir { entries }) => {
                    let child = *entries.get(component).ok_or(FSError::Unknown)?;
                    path_ids.push(child);
                }
                Some(FSContent::File { .. }) => {
                    Err(FSError::NoEntry)?;
                }
                None => {
                    remainder.get_or_insert(PathBuf::new()).push(component);
                }
            }
        }
        trace!("resolve_path: {:?} {:?}", path_ids, remainder);
        Ok((path_ids, remainder))
    }

    /// See the public api [Filesystem::resolve_last](Filesystem::resolve_last) for documentation.
    fn resolve_last(
        &self,
        root: FSEntryId,
        path: &Path,
    ) -> Result<FSEntryId, FSError> {
        let (ids, remain) = self.resolve_path(root, path)?;
        if remain.is_some() {
            return Err(FSError::Unknown);
        }
        return Ok(*ids.last().expect("Empty paths cannot occur"));
    }

    fn most_recent_historical_index(
        &self,
        index: FSEntryId,
    ) -> Result<Index, FSError> {
        trace!("most recent historical index: {:?}", index);
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
        trace!("result: {:?}", historical_index);
        trace!(
            "entry: {:?}",
            self.content[historical_index
                .unwrap()
                .historical_or(FSError::Unknown)?
                .content_id]
        );
        historical_index.ok_or(FSError::Unknown)
    }

    fn most_recent_snap(
        &self,
        index: FSEntryId,
    ) -> Result<SnapId, FSError> {
        Ok(self
            .most_recent_historical_index(index)?
            .historical_or(FSError::NotFound)?
            .snap_id)
    }

    fn most_recent_content_id(
        &self,
        index: FSEntryId,
    ) -> Result<ContentId, FSError> {
        Ok(self
            .most_recent_historical_index(index)?
            .historical_or(FSError::NotFound)?
            .content_id)
    }

    /// Returns the most recent entry mutably. If the entry is historical, a new copy is created and inserted,
    /// such that the entry may be modified in the context of the current snapshot.
    ///
    pub(super) fn most_recent_entry_mut(
        &mut self,
        fse_id: FSEntryId,
    ) -> Result<&mut Inode, FSError> {
        let historical_index = self
            .most_recent_historical_index(fse_id)?
            .historical_or(FSError::Unknown)?;
        if historical_index.snap_id < self.current_snap {
            // Historical. Cannot modify. Need to clone first.
            trace!("update historical index: {:?}", historical_index);
            let entry = self
                .content
                .get(historical_index.content_id)
                .expect("id must be valid");
            let new_entry = entry.clone();
            self.add_entry(fse_id, new_entry)?;
            trace!("updated historical index: {:?}", self.most_recent_historical_index(fse_id));
            let r = self.most_recent_entry_mut(fse_id);
            Ok(r?)
        } else {
            let r = self
                .content
                .get_mut(historical_index.content_id)
                .expect("id must be valid");
            trace!("most recent mut: {:?}", r);
            return Ok(r);
            Ok(self
                .content
                .get_mut(historical_index.content_id)
                .expect("id must be valid"))
        }
    }

    pub(super) fn most_recent_entry(
        &self,
        index: FSEntryId,
    ) -> Result<&Inode, FSError> {
        Ok(self
            .content
            .get(self.most_recent_content_id(index)?)
            .expect("The id must be valid"))
    }

    fn most_recent_entry_and_snap_id(
        &self,
        index: FSEntryId,
    ) -> Result<(&Inode, SnapId), FSError> {
        let historical_index = self
            .most_recent_historical_index(index)?
            .historical_or(FSError::Unknown)?;
        Ok((
            self.content
                .get(historical_index.content_id)
                .expect("The id must be valid"),
            historical_index.snap_id,
        ))
    }

    fn get_content_by_id(
        &self,
        content_id: ContentId,
    ) -> Result<&Inode, FSError> {
        self.content.get(content_id).ok_or(FSError::Unknown)
    }
    fn get_content_by_id_mut(
        &mut self,
        content_id: ContentId,
    ) -> Result<&mut Inode, FSError> {
        self.content.get_mut(content_id).ok_or(FSError::Unknown)
    }
    pub(super) fn print_tree(
        &self,
        root: FSEntryId,
        depth: Option<usize>,
    ) {
        let depth = depth.unwrap_or(0);
        let node = self.most_recent_entry(root).unwrap();
        println!(
            "{} todo",
            vec![' '; depth * 3]
                .into_iter()
                .chain("|-".chars())
                .collect::<String>() //node.name
        );
        match &node.content {
            Some(FSContent::File { .. }) => {}
            None => {}
            Some(FSContent::Dir { entries }) => {
                for entry in entries.values() {
                    self.print_tree(*entry, Some(depth + 1));
                }
            }
        }
    }

    pub(super) fn restore_and_discard(
        &mut self,
        snap_marker: MetaInfoSnapMarker,
    ) {
        if self.current_snap == 0 {
            panic!(""); // TODO: How to handle this case better? Maybe return error?
        }
        self.restore(snap_marker);
        self.current_snap -= 1;
    }

    pub(super) fn restore(
        &mut self,
        snap_marker: MetaInfoSnapMarker,
    ) {
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
pub(super) enum FSContent {
    File { blocks: BTreeSet<Block> },
    Dir { entries: BTreeMap<OsString, FSEntryId> },
}

#[derive(Clone, Debug)]
pub(super) struct Inode {
    pub(super) file_type: FSEntry,
    pub(super) stat: Stat,
    pub(super) content: Option<FSContent>,
}

impl Inode {
    pub(super) fn new(
        file_type: FSEntry,
        stat: Stat,
        content: Option<FSContent>,
    ) -> Self {
        Inode {
            file_type,
            stat,
            content,
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
    fn new(
        content_id: ContentId,
        snap_id: SnapId,
    ) -> Self {
        Self::Historical(HistoricalIndex { content_id, snap_id })
    }

    fn historical_or<E>(
        self,
        error: E,
    ) -> Result<HistoricalIndex, E> {
        match self {
            Index::Historical(historical_index) => Ok(historical_index),
            Index::Deleted => Err(error),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MetaInfoSnapMarker {
    content_marker: usize,
    next_fse_id: FSEntryId,
}
