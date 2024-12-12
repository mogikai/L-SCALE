use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomData;

use thiserror::Error;

use mogi_utilities::addresses::{
    Addressable,
    EVAddr,
};

#[derive(Debug, Error)]
pub enum JitCacheError {
    #[error("The requested block could not be found in the jit cache")]
    BlockNotFound,
}
/// Code is a pointer to a host virtual address that points to the start of executable jited code
#[repr(transparent)]
pub struct JitCode<'a>(*const c_void, PhantomData<&'a ()>);

impl<'a> JitCode<'a> {
    pub(crate) unsafe fn raw_ptr(&self) -> *const u8 {
        self.0.cast()
    }
}

impl<'a> From<*const c_void> for JitCode<'a> {
    fn from(value: *const c_void) -> Self {
        JitCode(value, Default::default())
    }
}

pub trait AsJitCode {
    fn as_jit_code(&self) -> JitCode;
}

#[derive(Default)]
pub struct JitCache<BlockT> {
    // TODO: Use page table instead of hash map?
    /// Code pages are boxed, because they must not move.
    cache: HashMap<EVAddr, BlockT>,
}

impl<BlockT> JitCache<BlockT>
where
    BlockT: AsJitCode + Addressable<EVAddr>,
{
    pub fn new() -> Self {
        Self {
            cache: Default::default(),
        }
    }

    /// Insert the provided block and return it's [EVAddr].
    pub fn populate(
        &mut self,
        block: BlockT,
    ) -> EVAddr {
        let addr = block.address();
        self.cache.insert(addr, block);
        addr
    }

    pub fn take(
        &mut self,
        evaddr: EVAddr,
    ) -> Result<BlockT, JitCacheError> {
        self.cache
            .remove(&evaddr)
            .ok_or(JitCacheError::BlockNotFound)
    }

    pub fn get_block_code(
        &self,
        evaddr: EVAddr,
    ) -> Result<JitCode, JitCacheError> {
        Ok(self
            .cache
            .get(&evaddr)
            .ok_or(JitCacheError::BlockNotFound)?
            .as_jit_code())
    }
}
