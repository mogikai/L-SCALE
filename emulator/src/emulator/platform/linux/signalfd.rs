use bitflags::Flags;

use crate::emulator::abstractions::file_descriptors::{FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;

#[derive(Debug, Clone)]
pub struct SignalFD {
    mask: u64,
}

//TODO build a trait

impl SignalFD {
    pub fn new(mask: u64) -> Self {
        SignalFD { mask }
    }
}

#[derive(Debug, Default)]
pub struct SignalFDOps {}

impl SignalFDOps {
    pub fn new() -> Self {
        SignalFDOps {}
    }
}

impl FileDescriptorOps<FileDescriptorData> for SignalFDOps {
    fn poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> Option<FileOperationsReady> {
        // TODO: Implement poll operation instead of never signaling
        Some(FileOperationsReady::empty())
    }

    fn supports_poll(
        &self,
        fd: &mut FileDescriptorData,
    ) -> bool {
        true
    }
}
