use std::fmt::Display;
use std::ops::Range;

use analysis::{LocalSliceId, ReferencedSlices};
use fnv::{FnvHashMap, FnvHashSet};
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::JIRSlice;

use super::{
    InstId,
    TargetBasicBlockInner,
};

mod dead_store_elimination;
mod analysis;

#[derive(Debug, Default)]
pub struct OptimizerOptions {
    /// If set to true, dead stores inside the basic block will be eliminated
    enable_dse: bool,
}

impl OptimizerOptions {
    pub fn with_enable_dse(
        mut self,
        enable_dse: bool,
    ) -> Self {
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

impl Display for OptimizerAncillary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Optimizer ancillary:")?;
        for (slice, slice_id) in self.slice_to_id.iter() {
            writeln!(f, "slice[{:6}] = {slice}", slice_id)?;
        }

        for (inst_id, slice_references) in self.inst_slice_references.iter() {
            writeln!(f, "inst[{inst_id}] writes: {:?}, reads: {:?}", slice_references.writes, slice_references.reads)?;
        }

        for (block_id, block_writes) in self.block_writes.items() {
            writeln!(f, "block[{block_id}] writes: {:?}", block_writes)?;
        }

        for (block_id, block_reads) in self.block_reads.items() {
            writeln!(f, "block[{block_id}] writes: {:?}", block_reads)?;
        }

        for (block_id, dead_stores) in self.dead_stores.items() {
            writeln!(f, "block[{block_id}] dead stores: {:?}", dead_stores)?;
        }

        Ok(())
    }
}

impl OptimizerOptions {
    fn new(enable_dse: bool) -> Self {
        Self { enable_dse }
    }
}

impl TargetBasicBlockInner {
    /// Calculate all necessary ancillary information necessary for the optimization passes.
    fn populate_ancillary(&mut self) -> ProcessorSpecResult {
        self.collect_slice_references()?;
        self.calculate_block_reads_and_writes()?;

        trace_jir!("{}", self.optimizer_ancillary);

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
