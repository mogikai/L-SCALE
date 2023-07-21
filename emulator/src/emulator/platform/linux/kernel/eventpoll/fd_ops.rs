use crate::emulator::abstractions::file_descriptors::{FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;

#[derive(Debug)]
pub(super) struct EpollOps {}

impl EpollOps {
    pub(super) fn new() -> Self {
        Self {}
    }
}

impl FileDescriptorOps<FileDescriptorData> for EpollOps {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        todo!()
    }

    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        true
    }
}
