use std::ops::Range;

use dfa::{LocalSliceId, ReferencedSlices};
use fnv::{FnvHashMap, FnvHashSet};
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use mogi_utilities::new_map_key;

use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::ProcessorSpecResult;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

use super::{
    InstId,
    TargetBasicBlockInner,
};

mod dead_store_elimination;
mod dfa;

#[derive(Debug, Default)]
pub struct OptimizerOptions {
    /// If set to true, dead stores inside the basic block will be eliminated
    enable_dse: bool,
}

impl OptimizerOptions {
    pub fn with_enable_dse(mut self, enable_dse: bool) -> Self {
        self.enable_dse = enable_dse;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct OptimizerAncillary {
    slice_to_id: FnvHashMap<JIRSlice, LocalSliceId>,
    slice_ids: PrimaryMap<LocalSliceId, JIRSlice>,
    inst_slice_references: FnvHashMap<InstId, ReferencedSlices>,

    live_ranges: SecondaryMap<LocalSliceId, Range<InstId>>,
    block_reads: SecondaryMap<JIRBlockId, FnvHashSet<LocalSliceId>>,
    block_writes: SecondaryMap<JIRBlockId, FnvHashSet<LocalSliceId>>,
    dead_stores: SecondaryMap<JIRBlockId, Vec<InstId>>,
}

impl OptimizerOptions {
    fn new(enable_dse: bool) -> Self {
        Self { enable_dse }
    }
}

impl TargetBasicBlockInner {
    /// Calculate all necessary ancillary information necessary for the optimization pass.
    fn populate_ancillary(&mut self) -> ProcessorSpecResult {
        self.collect_slice_references()?;
        self.calculate_block_reads_and_writes()?;

        Ok(())
    }

    pub fn run_optimization_passes(
        &mut self,
        options: OptimizerOptions,
    ) -> ProcessorSpecResult {
        self.populate_ancillary()?;

        // if options.enable_dse {
        //     self.eliminate_dead_stores()?;
        // }

        Ok(())
    }
}
