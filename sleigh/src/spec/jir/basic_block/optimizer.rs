use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Range;

use analysis::{
    LocalSliceId,
    OverlapGroupId,
    ReferencedSlices,
};
use copy_coalescing::CopyCoalescion;
use fnv::{
    FnvHashMap,
    FnvHashSet,
};
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::{JIRBlock, JIRBlockId};
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
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
mod copy_coalescing;
mod dead_store_elimination;
mod nop_elimination;

#[derive(Debug, Default)]
pub struct OptimizerOptions {
    /// If set to true, dead stores inside the basic block will be eliminated.
    enable_dse: bool,
    /// If set to true, copies will be coalesced.
    enable_copy_coalescing: bool,
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
}

#[derive(Debug)]
struct OverlapSlice {
    slice: JIRSlice,
}

#[derive(Clone, Debug, Default)]
pub(super) struct OptimizerAncillary {
    slice_to_id: FnvHashMap<JIRSlice, LocalSliceId>,
    slice_ids: PrimaryMap<LocalSliceId, JIRSlice>,

    // Slice overlap must be accounted for to correctly track reads and writes. This
    // is done by keeping track of overlap groups which are the set of neighboring
    // nodes for every slice. Consider for example:
    // [0-8] - [6-10]
    //   |  \   |
    // [0-4] [4-8]
    // In this case, the overlap groups for each node would be:
    // - [0-4]:  ([0-8]),                overlap_id=0
    // - [0-8]:  ([0-4], [6-10], [4-8]), overlap_id=1
    // - [6-10]: ([0-8], [4-8]),         overlap_id=2
    // - [4-8]:  ([0-8], [6-10]),        overlap_id=2
    overlap_groups: PrimaryMap<OverlapGroupId, FnvHashSet<LocalSliceId>>,
    group_ids: SecondaryMap<LocalSliceId, Option<OverlapGroupId>>,

    inst_slice_references: BTreeMap<InstId, ReferencedSlices>,

    // a list of ranges
    live_ranges: SecondaryMap<LocalSliceId, Vec<Range<InstId>>>,

    // store which instruction consumes the read
    block_reads: SecondaryMap<JIRBlockId, SecondaryMap<LocalSliceId, Option<InstId>>>,
    // store which instruction produces the write
    block_writes: SecondaryMap<JIRBlockId, SecondaryMap<LocalSliceId, Option<InstId>>>,

    // a list of dead-store instructions that can be eliminated
    dead_stores: SecondaryMap<JIRBlockId, Vec<InstId>>,

    // a single copy coalescion consists of eliminating a superfluous
    // copy instruction while rewriting the output of a previous instruction to
    // the input of the store target
    copy_coalescions: SecondaryMap<JIRBlockId, Vec<CopyCoalescion>>,
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

        for (inst_id, slice_references) in self.inst_slice_references.iter() {
            writeln!(f, "inst[{inst_id}] write: {:?}, reads: {:?}", slice_references.write, slice_references.reads)?;
        }

        for (overlap_group_id, overlap_slices) in self.overlap_groups.items() {
            writeln!(f, "overlap[{overlap_group_id}] slices: {:?}", overlap_slices)?;
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
    ) -> Self {
        Self {
            enable_dse,
            enable_copy_coalescing,
        }
    }
}

impl TargetBasicBlockInner {
    /// Calculate common ancillary information required by all passes.
    fn populate_ancillary(&mut self) -> ProcessorSpecResult {
        self.collect_slice_references()?;
        self.collect_slice_overlaps()?;
        self.calculate_block_reads_and_writes()?;

        trace_opt!("{}", self.optimizer_ancillary);

        Ok(())
    }

    pub fn run_optimization_passes(
        &mut self,
        options: OptimizerOptions,
    ) -> ProcessorSpecResult {
        self.populate_ancillary()?;
        let prev_count = self.jir_instructions.len();

        if options.enable_dse {
            let elim_count = self.eliminate_dead_stores()?;
            trace_opt!("Dead-store elimination eliminated {} instructions", elim_count);
        }

        if options.enable_copy_coalescing {
            self.calculate_copy_coalescions()?;
            for (block_id, copy_coalescions) in self.optimizer_ancillary.copy_coalescions.items() {
                trace_opt!("block[{block_id}] copy coalescions: {:#?}", copy_coalescions);
            }
            // let elim_count = self.coalesce_copies()?;
            trace_opt!("Copy coalescion eliminated {} instructions", elim_count);
        }

        // other passes don't delete instructions but rather replace them by NOP
        // which are all deleted in this final pass
        // self.eliminate_nop()?;

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
