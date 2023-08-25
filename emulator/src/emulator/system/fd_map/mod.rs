use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::ops::Add;
use std::rc::Rc;

use generational_arena::{Arena, Index};

use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::fd_map::error::FileDescriptorMapError;

pub mod error;
mod restorable;

pub type InternalFD = Index;

#[derive(Debug)]
pub struct FileDescriptor<D: Default + Restorable> {
    /// The data field contains all state data for the fds. Each system component has
    /// its own optional field in the data struct. The HandleOps implementation
    /// is responsible for checking whether the appropriate data field exists.
    /// If it does not, the user space application called the operation on an invalid
    /// handle, and the HandleOps implementation shall return an EBADF error.
    pub data: D,
    /// The ops implementation may contain pointers to system components.
    /// It must however be stateless. All data pertaining to state or
    /// similar fd related info must be contained in the data field.
    pub ops: Box<dyn FileDescriptorOps<D>>,
}

impl<D: Default + Restorable> FileDescriptor<D> {
    pub fn new(ops: Box<dyn FileDescriptorOps<D>>) -> Self {
        FileDescriptor {
            data: Default::default(),
            ops,
        }
    }
}

pub trait FdId: Copy + Clone + Ord + Default + From<u64> + Add<i32, Output = Self> {}

#[derive(Debug, Default)]
pub struct FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Restorable,
{
    files: Arena<RefCell<FileDescriptor<D>>>,
    file_descriptors: BTreeMap<TaskId, BTreeMap<FD, Index>>,
    internal_file_descriptors: BTreeMap<TaskId, Arena<Index>>,
    free_fds: BTreeMap<TaskId, Vec<FD>>,
    // TODO: Implement as a hook object that can be inserted to be called after the handle
    //       processed the call.
    // hooks: HashMap<TaskId, HashMap<FD, VecDeque<Box<dyn FileDescriptorOps<D>>>>>,
    snapshots: Vec<Snapshot<FD, TaskId>>,
}

#[derive(Debug)]
struct Snapshot<FD, TaskId>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
{
    file_descriptors: BTreeMap<TaskId, BTreeMap<FD, Index>>,
    internal_file_descriptors: BTreeMap<TaskId, Arena<Index>>,
    free_fds: BTreeMap<TaskId, Vec<FD>>,
}

impl<FD, TaskId, D> FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Restorable,
{
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn insert_at(
        &mut self,
        task_id: &TaskId,
        fd: FD,
        file_descriptor: FileDescriptor<D>,
    ) -> Result<FD, FileDescriptorMapError> {
        let index = self.files.insert(RefCell::new(file_descriptor));
        self.file_descriptors
            .entry(*task_id)
            .or_default()
            .try_insert(fd, index)
            .map_err(|_| {
                self.files.remove(index);
                FileDescriptorMapError::FDExists
            })?;
        Ok(fd)
    }

    pub fn insert(
        &mut self,
        task_id: &TaskId,
        file_descriptor: FileDescriptor<D>,
    ) -> Result<FD, FileDescriptorMapError> {
        let fd = self.get_free_fd(task_id);
        self.insert_at(task_id, fd, file_descriptor)
    }

    pub fn insert_internal(
        &mut self,
        task_id: &TaskId,
        file_descriptor: FileDescriptor<D>,
    ) -> Result<InternalFD, FileDescriptorMapError> {
        let index = self.files.insert(RefCell::new(file_descriptor));
        Ok(self
            .internal_file_descriptors
            .entry(*task_id)
            .or_default()
            .insert(index))
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
    ) -> Option<RefMut<FileDescriptor<D>>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .and_then(|fd_index| self.files.get(*fd_index).map(|fd| fd.borrow_mut()))
        })
    }

    pub fn get(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<Ref<FileDescriptor<D>>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .and_then(|fd_index| self.files.get(*fd_index).map(|fd| fd.borrow()))
        })
    }

    pub fn dup_internal(
        &mut self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Result<InternalFD, FileDescriptorMapError> {
        let file = self
            .file_descriptors
            .get(task_id)
            .and_then(|map| map.get(fd))
            .ok_or(FileDescriptorMapError::InvalidFD)?;
        Ok(self
            .internal_file_descriptors
            .entry(*task_id)
            .or_default()
            .insert(*file))
    }

    pub fn get_mut_internal(
        &mut self,
        task_id: &TaskId,
        ifd: &InternalFD,
    ) -> Option<RefMut<FileDescriptor<D>>> {
        self.internal_file_descriptors.get(task_id).and_then(|fds| {
            fds.get(*ifd)
                .and_then(|fd_index| self.files.get(*fd_index).map(|fd| fd.borrow_mut()))
        })
    }

    pub fn get_internal(
        &self,
        task_id: &TaskId,
        ifd: &InternalFD,
    ) -> Option<Ref<FileDescriptor<D>>> {
        self.internal_file_descriptors.get(task_id).and_then(|fds| {
            fds.get(*ifd)
                .and_then(|fd_index| self.files.get(*fd_index).map(|fd| fd.borrow()))
        })
    }

    fn get_free_fd(
        &mut self,
        task_id: &TaskId,
    ) -> FD {
        self.free_fds
            .entry(*task_id)
            .or_default()
            .pop()
            .unwrap_or_else(|| {
                *self
                    .file_descriptors
                    .entry(*task_id)
                    .or_default()
                    .keys()
                    .max()
                    .unwrap_or(&FD::from(2))
                    + 1
            })
    }
}
