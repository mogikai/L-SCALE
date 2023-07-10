use std::cell::{Ref, RefCell};
use std::rc::Rc;

use byteorder::ByteOrder;

use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::HandleOps;
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
pub struct SignalFDHandler {}

impl SignalFDHandler {
    pub fn new() -> Self { SignalFDHandler{} }
}

impl HandleOps for SignalFDHandler {}
