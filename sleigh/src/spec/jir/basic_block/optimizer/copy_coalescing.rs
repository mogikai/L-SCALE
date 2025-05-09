//! Copy coalescing optimizer.
//! This optimizer removes any superfluous copy instructions while rewriting the
//! output and input slices to account for this.
//! Any slices that are written to and not spilled can be replaced
//! by the value those slices are initialized with, i.e. either constants or other slices.
//! Non-copies or const loads (i.e. memory loads or operation results) cannot be optimized.

use mogi_utilities::index_map::SecondaryMap;

use crate::spec::ProcessorSpecResult;
use crate::spec::jir::basic_block::{
    InstId,
    TargetBasicBlockInner,
};
use crate::spec::jir::jcode::JIRInstruction;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::JIRValue;

#[derive(Debug, Clone)]
pub(super) struct Rewrite {
    // which instruction is being rewritten
    pub(super) target: InstId,
    // which input is being rewritten
    pub(super) input_idx: usize,
}

impl Rewrite {
    pub(super) fn new(
        target: InstId,
        input_idx: usize,
    ) -> Self {
        Self { target, input_idx }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CopyCoalescion {
    // the copy instruction the coalescion originates from
    pub(super) inst_id: InstId,
    // what value is the input being rewritten to? (either const or another slice)
    pub(super) value: JIRValue,

    // the instructions that make use of the copy result that need to be rewritten
    pub(super) rewrites: Vec<Rewrite>,
    // if `delete_copy_inst` is true, the copy instruction this coalescion derives from
    // can be safely deleted
    pub(super) delete_copy_inst: bool,
}

impl CopyCoalescion {
    pub(super) fn new(
        inst_id: InstId,
        value: JIRValue,
    ) -> Self {
        Self {
            inst_id,
            value,
            rewrites: Default::default(),
            delete_copy_inst: true,
        }
    }
}

impl TargetBasicBlockInner {
    pub(super) fn coalesce_copies(&mut self) -> ProcessorSpecResult<u64> {
        // rewrite instruction operands and replace superfluous copies by NOPs
        let mut elim_count = 0;

        for (block_id, copy_coalescions) in self.optimizer_ancillary.copy_coalescions.items() {
            for copy_coalescion in copy_coalescions.iter() {
                // apply rewrites
                for rewrite in copy_coalescion.rewrites.iter() {
                    let operand = self.jir_instructions[rewrite.target]
                        .operands_mut()
                        .skip(rewrite.input_idx)
                        .next()
                        .expect("This must exist");
                    *operand = copy_coalescion.value;
                }

                // delete
                if copy_coalescion.delete_copy_inst {
                    elim_count += 1;
                    self.jir_instructions[copy_coalescion.inst_id] = JIRInstruction::nop();
                }
            }
        }

        Ok(elim_count)
    }
}

mod test {
    use crate::jinst;
    use crate::spec::SpaceId;
    use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
    use crate::spec::jir::basic_block::optimizer::OptimizerOptions;
    use crate::spec::jir::common::JIRSize;
    use crate::spec::jir::jcode::{
        BinaryArgs,
        JIRInstruction,
        UnaryArgs,
    };
    use crate::spec::jir::values::{
        JIRConst,
        JIRSlice,
        JIRValue,
        VarnodeGroupId,
    };
    use mogi_utilities::index_map::MapKey;
    use std::rc::Rc;

    #[test]
    fn test_propagate_const_noelim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let const_0: JIRValue = JIRConst::new(123u128, JIRSize::try_from(1).unwrap()).into();

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(jinst! { slice_0 = const_0})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_copy_coalescing(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0], &jinst! { slice_0 = const_0 });
        assert_eq!(instructions[1], &jinst! { slice_1 = const_0 + slice_2 });
    }

    #[test]
    fn test_propagate_const_elim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let const_0: JIRValue = JIRConst::new(123u128, JIRSize::try_from(1).unwrap()).into();
        let const_1: JIRValue = JIRConst::new(456u128, JIRSize::try_from(1).unwrap()).into();

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(jinst! { slice_0 = const_0})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = const_1})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_copy_coalescing(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0], &jinst! { slice_1 = const_0 + slice_2 });
        assert_eq!(instructions[1], &jinst! { slice_0 = const_1 });
    }

    #[test]
    fn test_propagate_slice_noelim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = VarnodeGroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), space_id);
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_copy_coalescing(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0], &jinst! { slice_0 = slice_3 });
        assert_eq!(instructions[1], &jinst! { slice_1 = slice_3 + slice_2 });
    }

    #[test]
    fn test_propagate_slice_elim() {
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
            .push_jir_instruction(jinst! { slice_0 = slice_3})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_4})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(OptimizerOptions::default().with_enable_copy_coalescing(true))
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0], &jinst! { slice_1 = slice_3 + slice_2 });
        assert_eq!(instructions[1], &jinst! { slice_0 = slice_4 });
    }
}
