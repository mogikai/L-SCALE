//! Copy coalescing optimizer.
//! This optimizer removes any superfluous copy instructions while rewriting the
//! output and input slices to account for this.
//! Any slices that are written to and not spilled can be replaced
//! by the value those slices are initialized with, i.e. either constants or other slices.
//! Non-copies or const loads (i.e. memory loads or operation results) cannot be optimized.

use mogi_utilities::index_map::{
    MapKey,
    SecondaryMap,
};

use crate::jinst;
use crate::spec::jir::basic_block::{
    InstId,
    TargetBasicBlockInner,
};
use crate::spec::jir::jcode::{
    JIRInstruction,
    UnaryArgs,
};
use crate::spec::jir::values::{
    JIRConst,
    JIRGroup,
    JIRValue,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
    SpaceId,
};

use super::{
    LocalSliceId,
    OptimizerAncillary,
    OptimizerContext,
    OptimizerOptions,
};

#[derive(Debug, Clone)]
struct Rewrite {
    // which instruction is being rewritten
    target: InstId,
    // which input is being rewritten
    input_idx: usize,
}

impl Rewrite {
    fn new(
        target: InstId,
        input_idx: usize,
    ) -> Self {
        Self { target, input_idx }
    }
}

#[derive(Debug, Clone)]
struct CopyCoalescion {
    // the copy instruction the coalescion originates from
    inst_id: InstId,
    // what value is the input being rewritten to? (either const or another slice)
    value: JIRValue,
}

impl CopyCoalescion {
    fn new(
        inst_id: InstId,
        value: JIRValue,
    ) -> Self {
        Self { inst_id, value }
    }
}

impl TargetBasicBlockInner {
    /// Multiple optimizations are possible in a single forward pass:
    /// 1. copy coalescion eliminates all non-spilling copies by rewriting inputs and
    /// replacing copy instructions by NOP
    /// 2. const folding eliminates constant loads and evaluates all operations with
    /// constant input, replacing them by a single copy
    pub(super) fn forward_pass(
        &mut self,
        spec: &ProcessorSpecification,
        context: &OptimizerContext,
        options: &OptimizerOptions,
    ) -> ProcessorSpecResult<(u64, u64)> {
        let (mut elim_count, mut const_fold_count) = (0, 0);

        for (block_id, block) in self.jir_blocks.items() {
            let block = block.as_ref().expect("This must be some");

            let mut coalescions = SecondaryMap::<LocalSliceId, Option<CopyCoalescion>>::default();

            for inst_id in block.range.clone() {
                // apply rewrites
                for operand in self.jir_instructions[inst_id].operands_mut() {
                    let JIRValue::Slice(operand_slice) = operand else {
                        continue;
                    };

                    let operand_slice_id = self.optimizer_ancillary.slice_to_id[&*operand_slice];
                    if let Some(coalescion) = &coalescions[operand_slice_id] {
                        *operand = coalescion.value;
                    }
                }

                // invalidation of previous rewrites must be done unconditionally
                if let Some(write) = self.jir_instructions[inst_id].get_output() {
                    let write_id = self.optimizer_ancillary.slice_to_id[&write];

                    if let Some(coalescion) = &coalescions[write_id] {
                        // eliminate previous instruction that caused the coalescion
                        elim_count += 1;
                        self.jir_instructions[coalescion.inst_id] = JIRInstruction::nop();
                    }

                    // if an output is overwritten, we invalidate all coalescions of
                    // the output group
                    for overlap_candidate in &self.optimizer_ancillary.group_slices[write.group_id] {
                        coalescions[*overlap_candidate] = None;
                    }

                    // additionally, remove all coalescions where write occurs on the right-hand
                    // side (i.e. its slice id is part of the overlap group) since it's also
                    // invalidated if it's a slice
                    // TODO add second map instead of iterating every time? this is kinda bad rn
                    for slice_id in self.optimizer_ancillary.slice_ids.keys() {
                        let Some(some_coalescion) = &coalescions[slice_id] else {
                            continue;
                        };

                        if let JIRValue::Slice(value_slice) = &some_coalescion.value {
                            // TODO use exact overlap?
                            if value_slice.group_id == write.group_id {
                                coalescions[slice_id] = None;
                            }
                        }
                    }
                }

                // add new rewrites (constant fold, copy coalescion)

                if options.enable_constant_folding {
                    // if all input operands are known or we load from the constant
                    // space, we can evaluate the instruction and replace it
                    // by a simple copy which will then be propagated by the
                    // steps below
                    if let Some(output) = self.jir_instructions[inst_id].get_output()
                        && let Some(const_eval) = self.jir_instructions[inst_id].evaluate(context.const_space_id)
                    {
                        // instruction has an output and was successfully const-folded => replace by copy
                        trace_opt!(
                            "Folding consteval of [{inst_id}] = {} to {:?}...",
                            self.jir_instructions[inst_id],
                            const_eval
                        );

                        self.jir_instructions[inst_id] = jinst!(output = const_eval);
                        const_fold_count += 1;
                    }
                }

                if options.enable_copy_coalescing {
                    if let JIRInstruction::Copy(args) = &self.jir_instructions[inst_id] {
                        // a copy is stored for later optimization

                        let copy_output_id = self.optimizer_ancillary.slice_to_id[&args.output];
                        let new_coalescion = CopyCoalescion::new(inst_id, args.input.clone());
                        coalescions[copy_output_id] = Some(new_coalescion);
                    }
                }
            }

            // temporary instructions are never spilled so we can apply the remaining
            // eliminations for them
            // for (slice_id, coalescion) in coalescions.into_items() {
            //     let Some(coalescion) = coalescion else {
            //         continue;
            //     };

            //     let slice = self.optimizer_ancillary.slice_ids[slice_id];
            //     if matches!(self.jir_groups[slice.group_id], JIRGroup::TempGroup(_)) {
            //         self.jir_instructions[coalescion.inst_id] = JIRInstruction::nop();
            //         elim_count += 1;
            //     }
            // }
        }

        Ok((elim_count, const_fold_count))
    }
}

#[cfg(test)]
mod test {
    use crate::compiler::SleighCompilerResult;
    use crate::compiler::builder::SleighCompilerBuilder;
    use crate::jinst;
    use crate::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
    use crate::spec::jir::basic_block::optimizer::{
        OptimizerContext,
        OptimizerOptions,
    };
    use crate::spec::jir::common::JIRSize;
    use crate::spec::jir::jcode::{
        BinaryArgs,
        JIRInstruction,
        UnaryArgs,
    };
    use crate::spec::jir::values::{
        GroupId,
        JIRConst,
        JIRSlice,
        JIRValue,
        VarnodeGroupId,
    };
    use crate::spec::{
        ProcessorSpecification,
        SpaceId,
    };
    use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;
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
    fn test_propagate_const_noelim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let const_0: JIRValue = JIRConst::new(123u128, JIRSize::try_from(1).unwrap()).into();

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(jinst! { slice_0 = const_0})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default()
                    .with_enable_copy_coalescing(true)
                    .with_enable_nop_elimination(true)
                    .with_enable_constant_folding(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 4);
        assert_eq!(instructions[1], &jinst! { slice_0 = const_0 });
        assert_eq!(instructions[2], &jinst! { slice_1 = const_0 + slice_2 });
    }

    #[test]
    fn test_propagate_const_elim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let const_0: JIRValue = JIRConst::new(123u128, JIRSize::try_from(1).unwrap()).into();
        let const_1: JIRValue = JIRConst::new(456u128, JIRSize::try_from(1).unwrap()).into();

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
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
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default()
                    .with_enable_copy_coalescing(true)
                    .with_enable_nop_elimination(true)
                    .with_enable_constant_folding(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 4);
        assert_eq!(instructions[1], &jinst! { slice_1 = const_0 + slice_2 });
        assert_eq!(instructions[2], &jinst! { slice_0 = const_1 });
    }

    #[test]
    fn test_propagate_slice_noelim() {
        let space_id = SpaceId::try_from_index(1).unwrap();
        let group_id = GroupId::try_from_index(1).unwrap();

        let slice_0 = JIRSlice::new(space_id, group_id, 0, 1);
        let slice_1 = JIRSlice::new(space_id, group_id, 1, 1);
        let slice_2 = JIRSlice::new(space_id, group_id, 2, 1);
        let slice_3 = JIRSlice::new(space_id, group_id, 3, 1);

        let mut block_builder = TargetBasicBlockBuilder::new(0u64.into(), Default::default());
        block_builder.start_block().unwrap();

        block_builder
            .push_jir_instruction(jinst! { slice_0 = slice_3})
            .unwrap();
        block_builder
            .push_jir_instruction(jinst! { slice_1 = slice_0 + slice_2})
            .unwrap();

        let block_id = block_builder.finish_block().unwrap();
        let mut block = block_builder.finish(None).unwrap();
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default()
                    .with_enable_copy_coalescing(true)
                    .with_enable_constant_folding(true)
                    .with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 4);
        assert_eq!(instructions[1], &jinst! { slice_0 = slice_3 });
        assert_eq!(instructions[2], &jinst! { slice_1 = slice_3 + slice_2 });
    }

    #[test]
    fn test_propagate_slice_elim() {
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
        let spec = Rc::new(RefCell::new(SPEC.read().unwrap().clone()));

        Rc::get_mut(&mut block.0)
            .unwrap()
            .run_optimization_passes(
                &spec.borrow(),
                &OptimizerContext::new(SpaceId::try_from_index(0).unwrap(), SpaceId::try_from_index(2).unwrap()),
                &OptimizerOptions::default()
                    .with_enable_copy_coalescing(true)
                    .with_enable_constant_folding(true)
                    .with_enable_nop_elimination(true),
            )
            .unwrap();

        let instructions = block.instructions().collect::<Vec<_>>();
        assert_eq!(instructions.len(), 4);
        assert_eq!(instructions[1], &jinst! { slice_1 = slice_3 + slice_2 });
        assert_eq!(instructions[2], &jinst! { slice_0 = slice_4 });
    }
}
