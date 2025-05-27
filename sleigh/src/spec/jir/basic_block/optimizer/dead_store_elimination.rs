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

#[cfg(test)]
mod test {
    use crate::compiler::SleighCompilerResult;
    use crate::compiler::builder::SleighCompilerBuilder;
    use crate::jinst;
    use crate::spec::jir::jcode::BinaryArgs;
    use crate::spec::jir::jir_block::JIRBlockId;
    use crate::spec::jir::values::JIRValue;
    use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;
    use mogi_utilities::addresses::EVAddr;
    use mogi_utilities::index_map::MapKey;
    use std::cell::RefCell;
    use std::path::{
        Path,
        PathBuf,
    };
    use std::rc::Rc;
    use std::sync::{
        LazyLock,
        RwLock,
    };

    use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
    use crate::spec::jir::basic_block::optimizer::{
        OptimizerContext,
        OptimizerOptions,
    };
    use crate::spec::jir::jcode::JIRInstruction;
    use crate::spec::jir::values::{
        GroupId,
        JIRSlice,
    };
    use crate::spec::{
        ProcessorSpecification,
        SpaceId,
    };

    static SPEC: LazyLock<RwLock<ProcessorSpecification>> = LazyLock::new(|| {
        let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        spec_path.push("resources/test/x86/x86-64.slaspec");
        RwLock::new(build(&spec_path).unwrap())
    });

    fn build(spec_path: &Path) -> SleighCompilerResult<ProcessorSpecification> {
        let spec_dir = spec_path.parent().unwrap();
        let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
        let compiler = SleighCompilerBuilder::new()
            .preprocessor_factory(factory)
            .build();
        compiler.compile(spec_path)
    }

    #[test]
    fn test_remove_dead_store_nospill() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);
        let slice_4 = JIRSlice::new(space_id, group_id, 4, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = slice_0 + slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_1 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3 + slice_4 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 8);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_0 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_2 = slice_0 + slice_1 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_0 = slice_3 + slice_4 }
        );
        assert_eq!(
            instructions[5],
            &jinst! { slice_1 = slice_0 + slice_2 }
        );
        assert_eq!(
            instructions[6],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[7],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }

    #[test]
    fn test_remove_dead_store_spill() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);
        let slice_4 = JIRSlice::new(space_id, group_id, 4, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = slice_0 + slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_1 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3 + slice_4 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        // instructions[3] has been eliminated (the slice_0 = slice_1 + slice_2 operation)
        assert_eq!(instructions.len(), 7);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_0 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_2 = slice_0 + slice_1 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_0 = slice_3 + slice_4 }
        );
        assert_eq!(
            instructions[5],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[6],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }

    #[test]
    fn test_keep_live_stores() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_3 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_1 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = slice_1 + slice_0 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 9);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_3 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_0 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_1 = slice_0 + slice_2 }
        );
        assert_eq!(
            instructions[5],
            &jinst! { slice_1 = slice_1 + slice_2 }
        );
        assert_eq!(
            instructions[6],
            &jinst! { slice_2 = slice_1 + slice_0 }
        );
        assert_eq!(
            instructions[7],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[8],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }

    #[test]
    fn test_keep_live_stores_overlap() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 4);
        let slice_1 = JIRSlice::new(space_id, group_id, 0, 8);
        let slice_2 = JIRSlice::new(space_id, group_id, 4, 4);
        let slice_3 = JIRSlice::new(space_id, group_id, 4, 8);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_1 + slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = slice_0 + slice_0 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 7);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_1 = slice_1 + slice_1 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_2 = slice_0 + slice_0 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_1 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[5],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[6],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }

    #[test]
    fn test_keep_load() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_0 + slice_0 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_1 + slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = *[space_id]slice_0 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_3 = *[space_id]slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 8);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_0 = slice_0 + slice_0 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_1 = slice_1 + slice_1 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_2 = *[space_id]slice_0 }
        );
        assert_eq!(
            instructions[5],
            &jinst! { slice_3 = *[space_id]slice_1 }
        );
        assert_eq!(
            instructions[6],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[7],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }

    #[test]
    fn test_keep_store() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(JIRInstruction::InstructionStart(0u64.into()))
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_0 + slice_0 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_1 + slice_1 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_2 = slice_2 + slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_3 = slice_3 + slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { *[space_id]slice_0 = slice_2 })
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { *[space_id]slice_1 = slice_3 })
            .unwrap();
        block_builder
            .push_jir_instruction(JIRInstruction::InstructionEnd(0u64.into()))
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default().with_enable_dse(true).with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();

        assert_eq!(instructions.len(), 10);
        assert_eq!(
            instructions[0],
            &JIRInstruction::BlockStart(JIRBlockId::try_from_index(0).unwrap())
        );
        assert_eq!(
            instructions[1],
            &JIRInstruction::InstructionStart(0u64.into())
        );
        assert_eq!(
            instructions[2],
            &jinst! { slice_0 = slice_0 + slice_0 }
        );
        assert_eq!(
            instructions[3],
            &jinst! { slice_1 = slice_1 + slice_1 }
        );
        assert_eq!(
            instructions[4],
            &jinst! { slice_2 = slice_2 + slice_2 }
        );
        assert_eq!(
            instructions[5],
            &jinst! { slice_3 = slice_3 + slice_3 }
        );
        assert_eq!(
            instructions[6],
            &jinst! { *[space_id]slice_0 = slice_2 }
        );
        assert_eq!(
            instructions[7],
            &jinst! { *[space_id]slice_1 = slice_3 }
        );
        assert_eq!(
            instructions[8],
            &JIRInstruction::InstructionEnd(0u64.into())
        );
        assert_eq!(
            instructions[9],
            &JIRInstruction::BlockEnd(JIRBlockId::try_from_index(0).unwrap())
        );
    }
}
