use std::cell::{Ref, RefCell};
use std::rc::Rc;
use bitflags::Flags;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::file_descriptors::{FileDescriptorOps, FileOperationsReady};
use crate::emulator::platform::linux::fd::FileDescriptorData;
use crate::emulator::system::filesystem::{FSEntryId, Filesystem, Stat};
use crate::emulator::system::System;

#[derive(Debug, Clone)]
pub struct SignalFD {
    mask: u64
}

//TODO build a trait

impl SignalFD {
    pub fn new(
        mask: u64,
    ) -> Self {
        SignalFD {
            mask,
        }
    }
}

#[derive(Debug, Default)]
pub struct SignalFDOps {}

impl SignalFDOps {
    pub fn new() -> Self { SignalFDOps {} }
}

impl FileDescriptorOps<FileDescriptorData> for SignalFDOps {
    fn poll(&self, fd: &mut FileDescriptorData) -> Option<FileOperationsReady> {
        // TODO: Implement poll operation instead of never signaling
        Some(FileOperationsReady::empty())
    }

    fn supports_poll(&self, fd: &mut FileDescriptorData) -> bool {
        true
    }
}
