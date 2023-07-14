use std::collections::HashMap;
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::platform::linux::filedescriptor::FileDescriptor;
use crate::emulator::platform::linux::kernel::LinuxKernelError;
use crate::emulator::system::file_descriptors::fd_map::{FDPointer, FileDescriptorMap, FD};
use crate::emulator::system::System;

#[derive(Default)]
pub struct FDMap {
    handles: HashMap<FD, FDPointer>,
    free_fds: Vec<FD>,
}

impl FDMap {
    pub fn new() -> FDMap {
        FDMap {
            handles: Default::default(),
            free_fds: vec![],
        }
    }

    pub(crate) fn insert(
        &mut self,
        file_descriptor_map: &mut FileDescriptorMap,
        file_descriptor: FileDescriptor,
    ) -> FD {
        let fd = self.get_free_fd();
        self.insert_at(file_descriptor_map, fd, file_descriptor)
    }

    pub(crate) fn insert_at(
        &mut self,
        file_descriptor_map: &mut FileDescriptorMap,
        fd: FD,
        file_descriptor: FileDescriptor,
    ) -> FD {
        // TODO: Introduce error handling if fd is already used.
        let fd_pointer = file_descriptor_map.insert(file_descriptor);
        self.handles.try_insert(fd, fd_pointer).unwrap();
        fd
    }

    pub(crate) fn get_mut<'a>(
        &'a mut self,
        file_descriptor_map: &'a mut FileDescriptorMap,
        fd: &FD,
    ) -> Option<&mut FileDescriptor> {
        self.handles.get(fd).map(move |fd_pointer| {
            file_descriptor_map
                .get_mut(fd_pointer)
                .expect("The fd_pointer must always point to a valid fd. This is an implementation error")
        })
    }

    pub(crate) fn remove(
        &mut self,
        file_descriptor_map: &mut FileDescriptorMap,
        fd: &FD,
    ) -> Result<(), LinuxKernelError> {
        self.handles
            .get(fd)
            .ok_or(LinuxKernelError::EBADF)
            .map(|fd_pointer| {
                file_descriptor_map.decrease_refcount(fd_pointer).unwrap();
            })
    }

    fn get_free_fd(&mut self) -> FD {
        self.free_fds
            .pop()
            .unwrap_or_else(|| *self.handles.keys().max().unwrap_or(&FD::from(2)) + 1)
    }
}
