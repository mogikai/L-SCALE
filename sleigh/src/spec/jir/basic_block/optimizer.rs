use std::fmt::Display;
use std::ops::Range;

use fnv::{FnvHashMap, FnvHashSet};
use mogi_utilities::index_map::{PrimaryMap, SecondaryMap};
use mogi_utilities::new_map_key;

use crate::spec::jir::jir_block::{
    JIRBlockId,
};
use crate::spec::jir::values::{
    GroupId, JIRSlice
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
    SpaceId,
};

use super::{
    InstId,
    TargetBasicBlockInner,
};

#[cfg(feature = "trace_opt")]
macro_rules! trace_opt {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}
#[cfg(not(feature = "trace_opt"))]
macro_rules! trace_opt {
    ($($arg:tt)*) => {};
}

mod analysis;
mod dead_store_elimination;
mod forward_pass;
mod nop_elimination;

#[derive(Debug)]
pub struct OptimizerContext {
    temp_space_id: SpaceId,
    const_space_id: SpaceId,
}

impl OptimizerContext {
    pub fn new(
        temp_space_id: SpaceId,
        const_space_id: SpaceId,
    ) -> Self {
        Self {
            temp_space_id,
            const_space_id,
        }
    }
}

#[derive(Debug, Default)]
pub struct OptimizerOptions {
    /// If set to true, dead stores inside the basic block will be eliminated.
    enable_dse: bool,
    /// If set to true, copies will be coalesced.
    enable_copy_coalescing: bool,
    /// If set to true, constants will be folded.
    enable_constant_folding: bool,
    /// If set to true, all NOPs generated during previous passes will be eliminated.
    enable_nop_elimination: bool,
}

impl OptimizerOptions {
    pub fn with_enable_dse(
        mut self,
        enable_dse: bool,
    ) -> Self {
        self.enable_dse = enable_dse;
        self
    }

    pub fn with_enable_copy_coalescing(
        mut self,
        enable_copy_coalescing: bool,
    ) -> Self {
        self.enable_copy_coalescing = enable_copy_coalescing;
        self
    }

    pub fn with_enable_constant_folding(
        mut self,
        enable_constant_folding: bool,
    ) -> Self {
        self.enable_constant_folding = enable_constant_folding;
        self
    }

    pub fn with_enable_nop_elimination(
        mut self,
        enable_nop_elimination: bool,
    ) -> Self {
        self.enable_nop_elimination = enable_nop_elimination;
        self
    }
}

#[derive(Debug)]
struct OverlapSlice {
    slice: JIRSlice,
}

new_map_key! { pub struct LocalSliceId; "LSID"; }

#[derive(Clone, Debug, Default)]
pub(super) struct OptimizerAncillary {
    slice_to_id: FnvHashMap<JIRSlice, LocalSliceId>,
    slice_ids: PrimaryMap<LocalSliceId, JIRSlice>,

    group_slices: SecondaryMap<GroupId, FnvHashSet<LocalSliceId>>,

    // a list of ranges
    live_ranges: SecondaryMap<LocalSliceId, Vec<Range<InstId>>>,

    // store which instruction consumes the read
    block_reads: SecondaryMap<JIRBlockId, SecondaryMap<LocalSliceId, Option<InstId>>>,
    // store which instruction produces the write
    block_writes: SecondaryMap<JIRBlockId, SecondaryMap<LocalSliceId, Option<InstId>>>,

    // a list of dead-store instructions that can be eliminated
    dead_stores: SecondaryMap<JIRBlockId, Vec<InstId>>,
}

impl Display for OptimizerAncillary {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "Optimizer ancillary:")?;
        for (slice, slice_id) in self.slice_to_id.iter() {
            writeln!(f, "slice[{:6}] = {slice}", slice_id)?;
        }

        for (group_id, group_slices) in self.group_slices.items() {
            writeln!(f, "group[{group_id}] slices: {:?}", group_slices)?;
        }

        for (block_id, block_writes) in self.block_writes.items() {
            writeln!(f, "block[{block_id}] writes: {:?}", block_writes)?;
        }

        for (block_id, block_reads) in self.block_reads.items() {
            writeln!(f, "block[{block_id}] reads: {:?}", block_reads)?;
        }

        for (block_id, dead_stores) in self.dead_stores.items() {
            writeln!(f, "block[{block_id}] dead stores: {:?}", dead_stores)?;
        }

        Ok(())
    }
}

impl OptimizerOptions {
    fn new(
        enable_dse: bool,
        enable_copy_coalescing: bool,
        enable_constant_folding: bool,
        enable_nop_elimination: bool,
    ) -> Self {
        Self {
            enable_dse,
            enable_copy_coalescing,
            enable_constant_folding,
            enable_nop_elimination,
        }
    }
}

impl TargetBasicBlockInner {
    /// Calculate common ancillary information required by all passes.
    fn populate_ancillary(
        &mut self,
        spec: &ProcessorSpecification,
    ) -> ProcessorSpecResult {
        self.collect_slices(spec)?;
        self.calculate_block_reads_and_writes(spec)?;

        trace_opt!("{}", self.optimizer_ancillary);

        Ok(())
    }

    pub fn run_optimization_passes(
        &mut self,
        spec: &ProcessorSpecification,
        context: &OptimizerContext,
        options: &OptimizerOptions,
    ) -> ProcessorSpecResult {
        self.populate_ancillary(spec)?;
        let prev_count = self.jir_instructions.len();

        if options.enable_dse {
            let elim_count = self.eliminate_dead_stores()?;
            trace_opt!("Dead-store elimination eliminated {} instructions", elim_count);
        }

        if options.enable_copy_coalescing {
            let (elim_count, const_fold_count) = self.forward_pass(spec, context, options)?;
            trace_opt!("Copy coalescion eliminated {} instructions, folded {} constants", elim_count, const_fold_count);
        }

        // other passes don't delete instructions but rather replace them by NOPs
        // which are all deleted in this final pass
        if options.enable_nop_elimination {
            let elim_count = self.eliminate_nop()?;
            trace_opt!("NOP elimination eliminated {} instructions", elim_count);
        }

        let after_count = self.jir_instructions.len();
        let total_elim_count = prev_count - after_count;
        trace_opt!(
            "Optimization eliminated {}/{} instructions ({}%)",
            total_elim_count,
            prev_count,
            total_elim_count as f64 / prev_count as f64 * 100.0
        );

        Ok(())
    }
}
