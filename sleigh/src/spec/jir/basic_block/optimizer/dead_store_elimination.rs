//! Dead store elimination optimizer.
//! This optimizer removes any stores that are never read.
//! Any varnode that is written to before it has been read is considered dead.
//! Varnodes that have not been read before the end of the block must always be
//! kept, since we don't know if they will be accessed later on.

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::TargetBasicBlockInner;
use crate::spec::jir::jcode::JIRInstruction;

impl TargetBasicBlockInner {
    pub(super) fn eliminate_dead_stores(&mut self) -> ProcessorSpecResult<u64> {
        let mut count = 0;
        for (_, dead_stores) in self.optimizer_ancillary.dead_stores.items() {
            for dead_store in dead_stores.iter() {
                count += 1;
                self.jir_instructions[*dead_store] = JIRInstruction::nop();
            }
        }

        Ok(count)
    }
}

mod test {
    use crate::spec::jir::jcode::BinaryArgs;
    use crate::spec::jir::values::JIRValue;
    use mogi_utilities::index_map::MapKey;
    use std::rc::Rc;

    use crate::spec::SpaceId;
    use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
    use crate::spec::jir::basic_block::optimizer::OptimizerOptions;
    use crate::spec::jir::jcode::JIRInstruction;
    use crate::spec::jir::values::{
        JIRSlice,
        VarnodeGroupId,
    };

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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_dse(true))
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
