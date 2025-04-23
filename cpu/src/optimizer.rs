use dead_store_elimination::eliminate_dead_stores;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;

use crate::jit::api::JITResult;

mod dead_store_elimination;
mod preemptive_flush_optimizer;

pub(crate) fn run_optimization_passes(block: TargetBasicBlock) -> JITResult<TargetBasicBlock> {
    let block = eliminate_dead_stores(block)?;

    Ok(block)
}
