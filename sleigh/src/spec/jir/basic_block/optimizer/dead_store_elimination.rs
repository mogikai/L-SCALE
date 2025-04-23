//! Dead store elimination optimizer.
//! This optimizer removes any stores that are never read.
//! Any varnode that is written to before it has been read is considered dead.
//! Varnodes that have not been read before the end of the block must always be
//! kept, since we don't know if they will be accessed later on.

use crate::spec::{jir::basic_block::TargetBasicBlockInner, ProcessorSpecResult};


impl TargetBasicBlockInner {
    pub(super) fn eliminate_dead_stores(&mut self) -> ProcessorSpecResult {
        // Instructions that store values that are neither written nor spilled can
        // be determined by a single reverse pass and discarded.

        Ok(())
    }
}
