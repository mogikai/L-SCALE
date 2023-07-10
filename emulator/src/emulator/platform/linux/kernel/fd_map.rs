use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Keys;
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::platform::linux::filedescriptor::FileDescriptor;

pub(crate) struct HandleMap {
    handles: HashMap<u64, (Box<dyn Handle>,FileDescriptor)>,
    free_fds: Vec<u64>,
    replacement_handles: HashMap<u64, VecDeque<Box<dyn Handle>>>,
}

impl HandleMap {
    pub(crate) fn new() -> Self {
        Self {
            handles: HashMap::new(),
            free_fds: Vec::new(),
            replacement_handles: HashMap::new(),
        }
    }
}

impl HandleMap {
    /// This function is used to add a replacement handle that is inserted instead of the actual handle
    /// for the first call that is associated with the supplied fd. If multiple replacements are added
    /// for the same fd, they will be used subsequently for each new insert that happens after the fd
    /// was closed, i.e. after it was removed
    /// # Arguments
    ///
    /// * `fd`: The fd the replacement is associated with.
    /// * `handle`: The replacement handle.
    ///
    /// returns: ()
    pub(crate) fn add_replacement_handle(
        &mut self,
        fd: u64,
        handle: Box<dyn Handle>,
    ) {
        // TODO: We need to be able to skip handles and only replace later ones
        let handles = self.replacement_handles.entry(fd).or_default();
        handles.push_front(handle);
    }

    // TODO: Need to implement dup. We will most likely have to move to RC or a different datastructure that can store
    //       references to its own members
    // TODO: Need to associate pid with fd as well. If we implement fork() we will need to be able to have the same fd
    //       for multiple processes. Only the close operation will then delete the fd for one of the processes.
    //       Maybe like this?
    //       (fd, pid) -> internal_fd_id
    //       internal_fd_id -> Box<dyn Handle>
    pub(crate) fn insert(
        &mut self,
        fd: u64,
        handle: (Box<dyn Handle>, FileDescriptor),
    ) -> Option<(Box<dyn Handle>, FileDescriptor)> {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        let (h, d) = handle;
        let h = self.replacement_handles
                .entry(fd)
                .or_default()
                .pop_back()
                .unwrap_or(h);
        self.handles.insert(
            fd, (h, d)
        )
    }

    pub(crate) fn add(
        &mut self,
        handle: (Box<dyn Handle>, FileDescriptor),
    ) -> u64 {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        let fd = self.get_free_fd();
        let old = self.insert(fd, handle);
        if old.is_some() {
            panic!("Add must only insert into unused keys");
        }
        fd
    }

    pub(crate) fn get_mut(
        &mut self,
        fd: &u64,
    ) -> Option<&mut (Box<dyn Handle>, FileDescriptor)> {
        self.handles.get_mut(fd)
    }

    pub(crate) fn keys(&self) -> Keys<u64, (Box<dyn Handle>, FileDescriptor)> {
        self.handles.keys()
    }

    pub(crate) fn get(
        &self,
        fd: &u64,
    ) -> Option<&(Box<dyn Handle>, FileDescriptor)> {
        self.handles.get(fd)
    }

    pub(crate) fn remove(
        &mut self,
        fd: &u64,
    ) -> Option<(Box<dyn Handle>, FileDescriptor)> {
        let old = self.handles.remove(fd);
        if old.is_some() {
            self.free_fds.push(*fd);
        }
        old
    }

    pub(crate) fn get_free_fd(&mut self) -> u64 {
        // TODO: Implement max fd handling (e.g. 1024?)
        self.free_fds
            .pop()
            .unwrap_or_else(|| *self.handles.keys().max().unwrap_or(&2) + 1)
    }
}
