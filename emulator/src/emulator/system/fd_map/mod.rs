use std::cell::{
    Ref,
    RefCell,
    RefMut,
};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::mem::ManuallyDrop;
use std::ops::{
    Add,
    Deref,
    DerefMut,
};
use std::rc::Rc;

use generational_arena::{
    Arena,
    Index,
};
use tracing::trace;

use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorError,
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::SocketAddress;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::fd_map::error::FileDescriptorMapError;
use crate::emulator::system::filesystem::Stat;
use crate::util::auto_key::lazy_clone_map::{
    LazyCloneMap,
    RemovedElement,
};
use crate::util::auto_key::restorable_map::RestorableHashMap;
use crate::util::auto_key::AutoKey;
use crate::util::rcow::RCow;

pub mod error;
mod restorable;

/// The internal file descriptors are used for internal file operations where a 'real'
/// file descriptor is not needed. These are never exposed to the user space application.
pub type InternalFD = Index;

pub trait DebugFileDescriptorOps: FileDescriptorOps + Debug {}
impl<T> DebugFileDescriptorOps for T where T: FileDescriptorOps + Debug {}

#[derive(Debug, Clone)]
pub struct FileDescriptor<D: Default + Clone> {
    /// The data field contains all state data for the fds. Each system component has
    /// its own optional field in the data struct. The HandleOps implementation
    /// is responsible for checking whether the appropriate data field exists.
    /// If it does not, the user space application called the operation on an invalid
    /// handle, and the HandleOps implementation shall return an EBADF error.
    /// The data type must implement an efficient clone operation to facilitate
    /// efficient state loading and restoring. This can be most easily achieved
    /// by performing copy on write cloning.
    pub data: D,
    /// The ops implementation may contain pointers to system components.
    /// It must however be stateless. All data pertaining to state or
    /// similar fd related info must be contained in the data field.
    ops: Rc<dyn DebugFileDescriptorOps<D = D>>,

    fd_count: usize,
}

impl<D: Default + Clone> FileDescriptor<D> {
    pub fn new(ops: Box<dyn DebugFileDescriptorOps<D = D>>) -> Self {
        FileDescriptor {
            data: Default::default(),
            ops: ops.into(),
            fd_count: 0,
        }
    }

    pub fn poll(
        &mut self,
        interest: FileOperationsReady,
    ) -> Option<FileOperationsReady> {
        self.ops.poll(&mut self.data, interest)
    }

    pub fn supports_poll(&mut self) -> bool {
        self.ops.supports_poll(&mut self.data)
    }

    fn close(&mut self) -> Result<(), FileDescriptorError> {
        self.fd_count = self
            .fd_count
            .checked_sub(1)
            .expect("fd_count would have underflowed. Programming error!");
        if self.fd_count == 0 {
            self.ops.close(&mut self.data)
        } else {
            Ok(())
        }
    }

    fn ref_count(&self) -> usize {
        self.fd_count
    }

    pub fn stat(&mut self) -> Result<Ref<'_, Stat>, FileDescriptorError> {
        self.ops.stat(&mut self.data)
    }

    pub fn read(
        &mut self,
        count: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        self.ops.read(&mut self.data, count)
    }

    pub fn pread(
        &mut self,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        self.ops.pread(&mut self.data, offset, length)
    }

    pub fn write(
        &mut self,
        data: &[u8],
    ) -> Result<usize, FileDescriptorError> {
        self.ops.write(&mut self.data, data)
    }

    pub fn bind(
        &mut self,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        self.ops.bind(&mut self.data, sockaddr)
    }

    pub fn getsockname(&mut self) -> Result<&SocketAddress, FileDescriptorError> {
        self.ops.getsockname(&mut self.data)
    }

    pub fn sendto(
        &mut self,
        data: &[u8],
        target: &Option<SocketAddress>,
    ) -> Result<usize, FileDescriptorError> {
        self.ops.sendto(&mut self.data, data, target)
    }

    pub fn recvmsg(
        &mut self,
        len: usize,
    ) -> Result<Vec<u8>, FileDescriptorError> {
        self.ops.recvmsg(&mut self.data, len)
    }

    pub fn recvfrom(
        &mut self,
        len: usize,
    ) -> Result<(Vec<u8>, SocketAddress), FileDescriptorError> {
        self.ops.recvfrom(&mut self.data, len)
    }

    pub fn connect(
        &mut self,
        sockaddr: SocketAddress,
    ) -> Result<(), FileDescriptorError> {
        self.ops.connect(&mut self.data, sockaddr)
    }

    pub fn setsockopt(&mut self) -> Result<(), FileDescriptorError> {
        self.ops.setsockopt(&mut self.data)
    }

    pub fn accept(&mut self) -> Result<(FileDescriptor<D>, SocketAddress), FileDescriptorError> {
        self.ops.accept(&mut self.data)
    }

    pub fn listen(
        &mut self,
        backlog: usize,
    ) -> Result<(), FileDescriptorError> {
        self.ops.listen(&mut self.data, backlog)
    }
}

pub trait FdId: Copy + Clone + Ord + Default + From<u64> + Add<i32, Output = Self> {}

#[derive(Debug, Default)]
pub struct FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    files: LazyCloneMap<FileDescriptor<D>>,
    file_descriptors: BTreeMap<TaskId, BTreeMap<FD, AutoKey>>,
    internal_file_descriptors: BTreeMap<TaskId, Arena<AutoKey>>,
    free_fds: BTreeMap<TaskId, Vec<FD>>,
    // TODO: Implement as a hook object that can be inserted to be called after the handle
    //       processed the call.
    // hooks: HashMap<TaskId, HashMap<FD, VecDeque<Box<dyn FileDescriptorOps<D>>>>>,
    snapshots: Vec<Snapshot<FD, TaskId, D>>,
}

#[derive(Debug)]
pub struct TemporaryRemoveGuard<'a, FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    guarded_map: &'a mut FileDescriptorMap<FD, TaskId, D>,
    temporarily_removed: ManuallyDrop<RemovedElement<FileDescriptor<D>>>,
}

impl<'a, FD, TaskId, D> TemporaryRemoveGuard<'a, FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    pub fn with_fd_map<ActionClosureT, ClosureReturnT>(
        &mut self,
        mut action: ActionClosureT,
    ) -> ClosureReturnT
    where
        ActionClosureT: FnMut(&mut FileDescriptorMap<FD, TaskId, D>, &mut FileDescriptor<D>) -> ClosureReturnT,
    {
        action(self.guarded_map, &mut self.temporarily_removed)
    }
}

impl<'a, FD, TaskId, D> Deref for TemporaryRemoveGuard<'a, FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    type Target = FileDescriptor<D>;

    fn deref(&self) -> &Self::Target {
        self.temporarily_removed.deref()
    }
}

impl<'a, FD, TaskId, D> DerefMut for TemporaryRemoveGuard<'a, FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.temporarily_removed.deref_mut()
    }
}

impl<'a, FD, TaskId, D> Drop for TemporaryRemoveGuard<'a, FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    fn drop(&mut self) {
        // Safe because no accesses can occur after this point
        unsafe {
            self.guarded_map
                .files
                .reinsert(ManuallyDrop::take(&mut self.temporarily_removed));
        }
    }
}

#[derive(Debug)]
struct Snapshot<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    files: LazyCloneMap<FileDescriptor<D>>,
    file_descriptors: BTreeMap<TaskId, BTreeMap<FD, AutoKey>>,
    internal_file_descriptors: BTreeMap<TaskId, Arena<AutoKey>>,
    free_fds: BTreeMap<TaskId, Vec<FD>>,
}

impl<FD, TaskId, D> FileDescriptorMap<FD, TaskId, D>
where
    FD: FdId,
    TaskId: Ord + Copy + Clone + Default,
    D: Default + Clone,
{
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn insert_at(
        &mut self,
        task_id: &TaskId,
        fd: FD,
        mut file_descriptor: FileDescriptor<D>,
    ) -> Result<FD, FileDescriptorMapError> {
        file_descriptor.fd_count += 1;
        let index = self.files.insert(file_descriptor);
        self.file_descriptors
            .entry(*task_id)
            .or_default()
            .try_insert(fd, index)
            .map_err(|_| {
                self.files.remove(&index);
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
        let index = self.files.insert(file_descriptor);
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
        let removed_index = self
            .file_descriptors
            .get_mut(task_id)
            .and_then(|map| map.remove(fd))
            .ok_or(FileDescriptorMapError::InvalidFD)?;
        let file = self.files.get_mut(&removed_index).unwrap();
        file.close().unwrap();
        if file.ref_count() == 0 {
            self.files.remove(&removed_index);
        }
        Ok(())
    }

    pub fn get_mut(
        &mut self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<&mut FileDescriptor<D>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .and_then(|fd_index| self.files.get_mut(fd_index))
        })
    }

    /// FIXME: THIS BREAKS INVARIANTS IF PUT_BACK IS NOT CALLED. NEED TO FIND A MORE SOPHISTICATED WAY
    pub fn remove_temporarily<'a>(
        &'a mut self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<TemporaryRemoveGuard<'a, FD, TaskId, D>> {
        let removed = self
            .file_descriptors
            .get(task_id)
            .and_then(|map| map.get(fd).and_then(|fd_index| self.files.remove(fd_index)));
        removed.and_then(|removed| {
            Some(TemporaryRemoveGuard {
                guarded_map: self,
                temporarily_removed: ManuallyDrop::new(removed),
            })
        })
    }

    pub fn get(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<&FileDescriptor<D>> {
        self.file_descriptors
            .get(task_id)
            .and_then(|map| map.get(fd).and_then(|fd_index| self.files.get(fd_index)))
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
    ) -> Option<&mut FileDescriptor<D>> {
        self.internal_file_descriptors.get(task_id).and_then(|fds| {
            fds.get(*ifd)
                .and_then(|fd_index| self.files.get_mut(fd_index))
        })
    }

    pub fn get_internal(
        &self,
        task_id: &TaskId,
        ifd: &InternalFD,
    ) -> Option<&FileDescriptor<D>> {
        self.internal_file_descriptors
            .get(task_id)
            .and_then(|fds| fds.get(*ifd).and_then(|fd_index| self.files.get(fd_index)))
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
