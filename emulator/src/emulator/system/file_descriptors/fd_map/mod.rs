use std::collections::{HashMap, VecDeque};
use std::ops::Add;

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

#[derive(Debug)]
pub struct FDPointer(generational_arena::Index);

#[derive(Debug)]
struct FileDescriptorMapEntry {
    file_descriptor: FileDescriptor,
    refcount: usize,
}

impl From<FileDescriptor> for FileDescriptorMapEntry {
    fn from(file_descriptor: FileDescriptor) -> Self {
        Self {
            file_descriptor,
            refcount: 1,
        }
    }
}

#[derive(Debug)]
pub struct FileDescriptorMap {
    file_descriptors: generational_arena::Arena<FileDescriptorMapEntry>,
    // TODO: Implement as a hook object that can be inserted to be called after the handle
    //       processed the call.
    replacement_handles: HashMap<TaskId, HashMap<FD, VecDeque<Box<dyn FileDescriptorOps>>>>,
}

impl FileDescriptorMap {
    pub fn new() -> Self {
        Self {
            file_descriptors: Default::default(),
            replacement_handles: Default::default(),
        }
    }

    pub fn insert(
        &mut self,
        file_descriptor: FileDescriptor,
    ) -> FDPointer {
        FDPointer(self.file_descriptors.insert(file_descriptor.into()))
    }

    pub fn get_mut(
        &mut self,
        fd_pointer: &FDPointer,
    ) -> Option<&mut FileDescriptor> {
        self.file_descriptors
            .get_mut(fd_pointer.0)
            .map(|entry| &mut entry.file_descriptor)
    }

    pub fn get(
        &self,
        fd_pointer: &FDPointer,
    ) -> Option<&FileDescriptor> {
        self.file_descriptors
            .get(fd_pointer.0)
            .map(|entry| &entry.file_descriptor)
    }

    /// Increase the reference count for the file descriptor pointed to by fd_pointer.
    /// For example, a dup call will create a new FD that points to the same file_descriptor,
    /// increasing its refcount by one.
    pub fn increase_refcount(
        &mut self,
        fd_pointer: &FDPointer,
    ) {
        self.file_descriptors
            .get_mut(fd_pointer.0)
            .map(|entry| entry.refcount += 1);
    }

    /// Decreases the refcount for the file descriptor pointed to by fd_pointer.
    /// Once the refcount reaches zero, i.e. all FDs pointing to this file_descriptor
    /// have been closed, the file_descriptor is removed, and any remaining pointers
    /// to the FD (can only happen due to programming error) will result in an error
    /// upon trying to access the pointer.
    pub fn decrease_refcount(
        &mut self,
        fd_pointer: &FDPointer,
    ) -> Result<(), FileDescriptorMapError> {
        let file_descriptor = self
            .file_descriptors
            .get_mut(fd_pointer.0)
            .ok_or(FileDescriptorMapError::InvalidFDPointer)?;

        assert!(file_descriptor.refcount >= 1);
        file_descriptor.refcount -= 1;
        if file_descriptor.refcount == 0 {
            self.file_descriptors.remove(fd_pointer.0);
        }
        Ok(())
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
