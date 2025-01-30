use crate::lifter::{
    LifterError,
    LifterResult,
};
use crate::spec::jir::basic_block::TargetBasicBlock;
use imbl::HashMap;
use imbl::hashmap::Entry;
use mogi_utilities::addresses::EVAddr;

/// The lifter cache contains all the translated instructions and blocks.
/// The instruction and target block maps are copy on write datastructures.
#[derive(Debug, Default, Clone)]
pub struct LifterCache {
    /// Entire blocks of the target that were lifted by lift_block.
    target_blocks: HashMap<EVAddr, TargetBasicBlock>,
}

impl LifterCache {
    /// Caches a lifted block. If the address is already occupied this will result in an error.
    /// On successful insertion, a reference to the inserted block is returned.
    ///
    /// If the lifter encounters the same address at a later point in time,
    /// it will retrieve the already translated block. If the InstructionSource has new bytes for
    /// the requested block, it has to signal this to the lifter, such that it can discard the cached entry.
    /// TODO: Maybe it might make sense to add a `register_invalidate_cache_hook` method to the InstructionSource
    ///       trait, such that the source can directly invalidate the cache entries if the data is overridden.
    pub fn cache_block(
        &mut self,
        addr: EVAddr,
        block: TargetBasicBlock,
    ) -> LifterResult<&TargetBasicBlock> {
        match self.target_blocks.entry(addr) {
            Entry::Occupied(_) => Err(LifterError::BlockAlreadyCached(addr)),
            Entry::Vacant(entry) => Ok(entry.insert(block)),
        }
    }

    pub fn flush_block(
        &mut self,
        addr: EVAddr,
    ) -> LifterResult {
        self.target_blocks.remove(&addr);
        Ok(())
    }

    pub fn fetch_block(
        &self,
        addr: EVAddr,
    ) -> LifterResult<&TargetBasicBlock> {
        self.target_blocks
            .get(&addr)
            .ok_or(LifterError::BlockAlreadyCached(addr))
    }

    pub fn is_block_cached(
        &self,
        addr: EVAddr,
    ) -> bool {
        self.target_blocks.contains_key(&addr)
    }
}
