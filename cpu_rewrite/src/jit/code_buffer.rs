use crate::jit::api::CustomOpArgs;
use libc::{
    MADV_HUGEPAGE,
    MAP_ANONYMOUS,
    MAP_PRIVATE,
    PROT_EXEC,
    PROT_READ,
    PROT_WRITE,
};
use mogi_utilities::addresses::EVAddr;
use std::collections::HashMap;
use std::ptr::{
    NonNull,
    null_mut,
};
use thiserror::Error;

const GiB: usize = 1 << 30;

#[derive(Debug, Error)]
pub enum CodeBufferError {
    #[error("Failed to allocate memory for code buffer")]
    AllocationFailure,
}

/// A compiled basic block. Points to a region inside the executable buffer.
/// The compiled block also contains a list of arguments for any custom op calls.
struct CompiledBasicBlock {
    code: NonNull<u8>,
    size: usize,
    custom_op_args: Vec<Box<CustomOpArgs>>,
}

pub struct CodeBuffer {
    buf: NonNull<u8>,
    size: usize,
    /// Keeps track of the blocks we have already compiled.
    blocks: HashMap<EVAddr, CompiledBasicBlock>,
}

impl CodeBuffer {
    pub fn new() -> Result<Self, CodeBufferError> {
        let size = 1 * GiB;
        // This is safe, since we return an error on failure.
        let buf = unsafe {
            let buf =
                libc::mmap(null_mut(), size, PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
            if libc::madvise(buf, size, MADV_HUGEPAGE) != 0 {
                Err(CodeBufferError::AllocationFailure)?
            }
            NonNull::new(buf as *mut u8).ok_or(CodeBufferError::AllocationFailure)?
        };
        Ok(Self {
            buf,
            size,
            blocks: Default::default(),
        })
    }
}

impl AsRef<[u8]> for CodeBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.buf.as_ptr(), self.size) }
    }
}

impl AsMut<[u8]> for CodeBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.buf.as_ptr(), self.size) }
    }
}
