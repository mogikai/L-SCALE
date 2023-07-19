use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::Add;
use std::rc::Rc;

use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::system::file_descriptors::fd_map::error::FileDescriptorMapError;

pub mod error;

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
        self.file_descriptors.entry(*task_id).or_insert(Default::default()).try_insert(fd, Rc::new(RefCell::new(file_descriptor))).map_err(|_| FileDescriptorMapError::FDExists)?;
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
        self.file_descriptors.get_mut(task_id).and_then(|map| {
            map.remove(fd).map(|_| ())
        }).ok_or(FileDescriptorMapError::InvalidFD)
    }

    pub fn get_mut(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<RefMut<FileDescriptor>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .and_then(|file_descriptor| file_descriptor.try_borrow_mut().ok())
        })
    }

    pub fn get(
        &self,
        task_id: &TaskId,
        fd: &FD,
    ) -> Option<Ref<FileDescriptor>> {
        self.file_descriptors.get(task_id).and_then(|map| {
            map.get(fd)
                .and_then(|file_descriptor| file_descriptor.try_borrow().ok())
        })
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

// impl FileDescriptorMap {
//     pub(crate) fn new() -> Self {
//         Self {
//             handles: HashMap::new(),
//             free_fds: Vec::new(),
//             replacement_handles: HashMap::new(),
//         }
//     }
// }
//
// impl FileDescriptorMap {
//     /// This function is used to add a replacement handle that is inserted instead of the actual handle
//     /// for the first call that is associated with the supplied fd. If multiple replacements are added
//     /// for the same fd, they will be used subsequently for each new insert that happens after the fd
//     /// was closed, i.e. after it was removed
//     /// # Arguments
//     ///
//     /// * `fd`: The fd the replacement is associated with.
//     /// * `handle`: The replacement handle.
//     ///
//     /// returns: ()
//     pub(crate) fn add_replacement_handle(
//         &mut self,
//         fd: u64,
//         handle: Box<dyn FileDescriptorOps>,
//     ) {
//         // TODO: We need to be able to skip handles and only replace later ones
//         let handles = self.replacement_handles.entry(fd).or_default();
//         handles.push_front(handle);
//     }
//
//     // TODO: Need to implement dup. We will most likely have to move to RC or a different datastructure that can store
//     //       references to its own members
//     // TODO: Need to associate pid with fd as well. If we implement fork() we will need to be able to have the same fd
//     //       for multiple processes. Only the close operation will then delete the fd for one of the processes.
//     //       Maybe like this?
//     //       (fd, pid) -> internal_fd_id
//     //       internal_fd_id -> Box<dyn Handle>
//     pub(crate) fn insert(
//         &mut self,
//         fd: u64,
//         mut handle: FileDescriptor,
//     ) -> Option<FileDescriptor> {
//         // Insert the handle if no replacement exists. Otherwise insert the replacement
//         handle.ops = self.replacement_handles
//             .entry(fd)
//             .or_default()
//             .pop_back()
//             .unwrap_or(handle.ops);
//         self.handles.insert(
//             fd, handle,
//         )
//     }
//
//     pub(crate) fn add(
//         &mut self,
//         handle: FileDescriptor,
//     ) -> u64 {
//         // Insert the handle if no replacement exists. Otherwise insert the replacement
//         let fd = self.get_free_fd();
//         let old = self.insert(fd, handle);
//         if old.is_some() {
//             panic!("Add must only insert into unused keys");
//         }
//         fd
//     }
//
//     pub(crate) fn get_mut(
//         &mut self,
//         fd: &u64,
//     ) -> Option<&mut FileDescriptor> {
//         self.handles.get_mut(fd)
//     }
//
//     pub(crate) fn keys(&self) -> Keys<u64, FileDescriptor> {
//         self.handles.keys()
//     }
//
//     pub(crate) fn get(
//         &self,
//         fd: &u64,
//     ) -> Option<&FileDescriptor> {
//         self.handles.get(fd)
//     }
//
//     pub(crate) fn remove(
//         &mut self,
//         fd: &u64,
//     ) -> Option<FileDescriptor> {
//         let old = self.handles.remove(fd);
//         if old.is_some() {
//             self.free_fds.push(*fd);
//         }
//         old
//     }
//
//     pub(crate) fn get_free_fd(&mut self) -> u64 {
//         // TODO: Implement max fd handling (e.g. 1024?)
//         self.free_fds
//             .pop()
//             .unwrap_or_else(|| *self.handles.keys().max().unwrap_or(&2) + 1)
//     }
// }
