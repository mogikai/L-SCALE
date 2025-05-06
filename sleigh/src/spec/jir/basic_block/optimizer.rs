use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Range;

use analysis::{
    LocalSliceId,
    OverlapGroupId,
    ReferencedSlices,
};
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
use crate::spec::jir::jir_block::JIRBlockId;
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
mod dead_store_elimination;

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

    live_ranges: SecondaryMap<LocalSliceId, Range<InstId>>,
    block_reads: SecondaryMap<JIRBlockId, FnvHashSet<LocalSliceId>>,
    block_writes: SecondaryMap<JIRBlockId, FnvHashSet<LocalSliceId>>,
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

        for (inst_id, slice_references) in self.inst_slice_references.iter() {
            writeln!(f, "inst[{inst_id}] writes: {:?}, reads: {:?}", slice_references.writes, slice_references.reads)?;
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
    fn new(enable_dse: bool) -> Self {
        Self { enable_dse }
    }
}

impl TargetBasicBlockInner {
    /// Calculate all necessary ancillary information necessary for the optimization passes.
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

        if options.enable_dse {
            self.eliminate_dead_stores()?;
        }

        Ok(())
    }
}

mod test {
    use super::*;
    use crate::spec::SpaceId;
    use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
    use crate::spec::jir::jcode::BinaryArgs;
    use crate::spec::jir::values::VarnodeGroupId;
    use mogi_utilities::index_map::MapKey;
    use std::rc::Rc;

    #[test]
    fn test_remove_dead_store_nospill() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);
        let slice_4 = JIRSlice::new(space_id, group_id, 4, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_1.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_4.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_0.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 4);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_4.into()
            })
        );
        assert_eq!(
            instructions[3],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_0.into(),
                input_b: slice_2.into()
            })
        );
    }

    #[test]
    fn test_remove_dead_store_spill() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);
        let slice_4 = JIRSlice::new(space_id, group_id, 4, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_1.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_4.into(),
            }))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        // instructions[2] has been eliminated
        assert_eq!(instructions.len(), 3);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_4.into()
            })
        );
    }

    #[test]
    fn test_keep_live_stores() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_3,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_0.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_1.into(),
                input_b: slice_0.into(),
            }))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 5);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_3,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_0.into(),
                input_b: slice_2.into()
            })
        );
        assert_eq!(
            instructions[3],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_2.into()
            })
        );
        assert_eq!(
            instructions[4],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_1.into(),
                input_b: slice_0.into()
            })
        );
    }

    #[test]
    fn test_keep_live_stores_overlap() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 4);
        let slice_1 = JIRSlice::new(space_id, group_id, 0, 8);
        let slice_2 = JIRSlice::new(space_id, group_id, 4, 4);
        let slice_3 = JIRSlice::new(space_id, group_id, 4, 8);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_0.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 3);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_0.into(),
                input_b: slice_0.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
    }

    #[test]
    fn test_keep_load() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Load {
                target: slice_2,
                address: JIRValue::Slice(slice_0),
                space: space_id,
            })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Load {
                target: slice_3,
                address: JIRValue::Slice(slice_1),
                space: space_id,
            })
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 4);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::Load {
                target: slice_2,
                address: JIRValue::Slice(slice_0),
                space: space_id
            }
        );
        assert_eq!(
            instructions[3],
            &JIRInstruction::Load {
                target: slice_3,
                address: JIRValue::Slice(slice_1),
                space: space_id
            }
        );
    }

    #[test]
    fn test_keep_store() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_2.into(),
                input_b: slice_2.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_3,
                input_a: slice_3.into(),
                input_b: slice_3.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Store {
                address: JIRValue::Slice(slice_0),
                value: slice_2.into(),
                space: space_id,
            })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Store {
                address: JIRValue::Slice(slice_1),
                value: slice_3.into(),
                space: space_id,
            })
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 6);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_2,
                input_a: slice_2.into(),
                input_b: slice_2.into()
            })
        );
        assert_eq!(
            instructions[3],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_3,
                input_a: slice_3.into(),
                input_b: slice_3.into()
            })
        );
        assert_eq!(
            instructions[4],
            &JIRInstruction::Store {
                address: JIRValue::Slice(slice_0),
                value: slice_2.into(),
                space: space_id
            }
        );
        assert_eq!(
            instructions[5],
            &JIRInstruction::Store {
                address: JIRValue::Slice(slice_1),
                value: slice_3.into(),
                space: space_id
            }
        );
    }

    #[test]
    fn test_eliminate_load() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into(),
            }))
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Load {
                target: slice_2,
                address: JIRValue::Slice(slice_0),
                space: space_id,
            })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::Load {
                target: slice_2,
                address: JIRValue::Slice(slice_1),
                space: space_id,
            })
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish().unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::new(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 3);
        assert_eq!(
            instructions[0],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_0,
                input_a: slice_0.into(),
                input_b: slice_0.into()
            })
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::IntAdd(BinaryArgs {
                output: slice_1,
                input_a: slice_1.into(),
                input_b: slice_1.into()
            })
        );
        assert_eq!(
            instructions[2],
            &JIRInstruction::Load {
                target: slice_2,
                address: JIRValue::Slice(slice_1),
                space: space_id
            }
        );
    }
}
