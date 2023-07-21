pub mod error;

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Add;
use std::rc::Rc;

use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::fd_map::error::FileDescriptorMapError;

#[derive(Hash, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct FD(i64);

impl Add<i32> for FD {
    type Output = FD;

    fn add(
        self,
        rhs: i32,
    ) -> Self::Output {
        FD(self.0 + i64::from(rhs))
    }
}

impl From<i32> for FD {
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

impl From<u64> for FD {
    fn from(value: u64) -> Self {
        FD(value as i64)
    }
}

impl From<FD> for u64 {
    fn from(value: FD) -> Self {
        value.0 as u64
    }
}

#[derive(Debug, Default)]
pub struct FileDescriptorMap {
    // TODO: Use generics instead of TaskId and FileDescriptor
    file_descriptors: BTreeMap<TaskId, BTreeMap<FD, Rc<RefCell<FileDescriptor>>>>,
    free_fds: BTreeMap<TaskId, Vec<FD>>,
    // TODO: Implement as a hook object that can be inserted to be called after the handle
    //       processed the call.
    replacement_handles: HashMap<TaskId, HashMap<FD, VecDeque<Box<dyn FileDescriptorOps>>>>,
}

impl FileDescriptorMap {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn insert_at(
        &mut self,
        task_id: &TaskId,
        fd: FD,
        file_descriptor: FileDescriptor,
    ) -> Result<FD, FileDescriptorMapError> {
        self.file_descriptors
            .entry(*task_id)
            .or_insert(Default::default())
            .try_insert(fd, Rc::new(RefCell::new(file_descriptor)))
            .map_err(|_| FileDescriptorMapError::FDExists)?;
        Ok(fd)
    }

    pub fn insert(
        &mut self,
        task_id: &TaskId,
        file_descriptor: FileDescriptor,
    ) -> Result<FD, FileDescriptorMapError> {
        let fd = self.get_free_fd(task_id);
        self.insert_at(task_id, fd, file_descriptor)
    }

    pub fn remove(
        &mut self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Result<(), FileDescriptorMapError> {
        self.file_descriptors
            .get_mut(task_id)
            .and_then(|map| map.remove(fd).map(|_| ()))
            .ok_or(FileDescriptorMapError::InvalidFD)
    }

    pub fn get_mut(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<RefMut<FileDescriptor>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .map(|file_descriptor| file_descriptor.borrow_mut())
        })
    }

    pub fn get(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<Ref<FileDescriptor>> {
        self.file_descriptors
            .get(task_id)
            .and_then(|map| map.get(fd).map(|file_descriptor| file_descriptor.borrow()))
    }

    fn get_free_fd(
        &mut self,
        task_id: &TaskId,
    ) -> FD {
        self.free_fds
            .entry(*task_id)
            .or_insert(vec![])
            .pop()
            .unwrap_or_else(|| {
                *self
                    .file_descriptors
                    .entry(*task_id)
                    .or_insert(Default::default())
                    .keys()
                    .max()
                    .unwrap_or(&FD::from(2))
                    + 1
            })
    }
}
