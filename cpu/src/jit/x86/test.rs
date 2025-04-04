use crate::jit::api::{
    JITTaskRuntimeContext,
    MogiJit,
};
use crate::jit::x86::X86MogiJit;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::SleighPreprocessorFactory;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use mogi_sleigh::spec::jir::basic_block::builder::TargetBasicBlockBuilder;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::JIRSlice;
use mogi_sleigh::spec::{
    ProcessorSpecification,
    SpaceId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::MapKey;
use num_traits::{
    FromBytes,
    ToBytes,
};
use std::cell::RefCell;
use std::fmt::{
    Debug,
    LowerHex,
};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;

pub(super) fn setup(
    debug: bool
) -> (X86MogiJit, Rc<RefCell<ProcessorSpecification>>, Box<<X86MogiJit as MogiJit>::RuntimeContext>) {
    let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_path.push("resources/test/x86/x86-64.slaspec");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let specification = Rc::new(RefCell::new(compiler.compile(spec_path.as_path()).unwrap()));
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let mut jit = X86MogiJit::new(specification.clone(), mmu.clone(), debug);
    let context = Box::new(jit.new_runtime_context());
    (jit, specification, context)
}

pub(super) fn set_slice<T: JITTaskRuntimeContext, V: ToBytes>(
    context: &mut T,
    slice: JIRSlice,
    value: V,
) {
    context
        .slice_mut(slice)
        .unwrap()
        .copy_from_slice(value.to_le_bytes().as_ref());
}

pub(super) fn get_slice_value_by_name<T: MogiJit, V: FromBytes>(
    jit: &T,
    context: &T::RuntimeContext,
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    name: &str,
) -> V
where
    for<'a> &'a [u8]: TryInto<V::Bytes>,
    for<'a> <&'a [u8] as TryInto<V::Bytes>>::Error: Debug,
    <V as FromBytes>::Bytes: Sized,
{
    let id = processor_spec.borrow().varnode_id_by_string(name).unwrap();
    let data = jit.varnode(&context, id).unwrap();
    FromBytes::from_le_bytes(&data.try_into().unwrap())
}

pub(super) fn get_slice_value<T: JITTaskRuntimeContext, V: FromBytes>(
    context: &T,
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    slice: JIRSlice,
) -> V
where
    for<'a> &'a [u8]: TryInto<V::Bytes>,
    for<'a> <&'a [u8] as TryInto<V::Bytes>>::Error: Debug,
    <V as FromBytes>::Bytes: Sized,
{
    let data = context.slice(slice).unwrap();
    FromBytes::from_le_bytes(&data.try_into().unwrap())
}

pub(super) fn get_reg_slice(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    name: &str,
) -> JIRSlice {
    processor_spec
        .borrow()
        .varnode_slice_by_string(name)
        .unwrap()
}

pub(super) fn make_basic_block_from_instructions(
    address: EVAddr,
    fallthrough: EVAddr,
    instructions: impl IntoIterator<Item = JIRInstruction>,
) -> TargetBasicBlock {
    let mut builder = TargetBasicBlockBuilder::new(address, SpaceId::try_from_index(0x41).unwrap());
    builder.set_fallthrough(fallthrough);
    builder.start_block().unwrap();
    for instruction in instructions.into_iter() {
        builder.push_jir_instruction(instruction).unwrap();
    }
    builder.finish_block().unwrap();
    builder.finish().unwrap()
}

pub(super) fn exec_instructions<T: MogiJit>(
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instructions: impl IntoIterator<Item = JIRInstruction>,
) {
    jit.compile_target_block(&make_basic_block_from_instructions(
        EVAddr::from(0x800100u64),
        EVAddr::from(0x800200u64),
        instructions,
    ))
    .unwrap();
    jit.set_instruction_pointer(context, EVAddr::from(0x800100u64))
        .unwrap();

    jit.execute(context, jit.instruction_pointer(&context).unwrap())
        .unwrap();
}

pub(super) fn test_instruction_generic<T, U1, U2, R>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: impl FnOnce(JIRSlice, JIRSlice, JIRSlice, U1, U2) -> (JIRInstruction, JIRSlice),
    verify_op: impl FnOnce(U1, U2) -> R,
    reg_out: &str,
    reg_a: &str,
    reg_b: &str,
    a: U1,
    b: U2,
) where
    T: MogiJit,
    U1: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U1 as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U1 as FromBytes>::Bytes>>::Error: Debug,
    <U1 as FromBytes>::Bytes: Sized,
    U2: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U2 as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U2 as FromBytes>::Bytes>>::Error: Debug,
    <U2 as FromBytes>::Bytes: Sized,
    R: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<R as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<R as FromBytes>::Bytes>>::Error: Debug,
    <R as FromBytes>::Bytes: Sized,
{
    let result = get_reg_slice(&processor_spec, reg_out);
    let slice_a = get_reg_slice(&processor_spec, reg_a);
    let slice_b = get_reg_slice(&processor_spec, reg_b);
    set_slice(context, slice_a, a);
    set_slice(context, slice_b, b);

    let (instruction, result) = instruction_builder(result, slice_a, slice_b, a, b);
    let instructions = vec![
        //
        instruction,
    ];
    exec_instructions(jit, context, instructions);
    let actual: R = get_slice_value(context, &processor_spec, result);

    let expected = verify_op(a, b);
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}, {:#x}", actual, expected, a, b);

    // Check that r8 and r9 have not been touched
    let a_value: U1 = get_slice_value(context, &processor_spec, slice_a);
    let b_value: U2 = get_slice_value(context, &processor_spec, slice_b);
    if result != slice_a {
        assert_eq!(a_value, a);
    }
    if result != slice_b {
        assert_eq!(b_value, b);
    }
}

/// Test all combinations of values when iterating over the type, using the provided step size.
pub(super) fn test_multiple_binary_instruction_generic<T, U, R, B, V>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: B,
    verify_op: V,
    value_iter: impl IntoIterator<Item = U> + Clone,
    reg_out: &str,
    reg_a: &str,
    reg_b: &str,
) where
    T: MogiJit,
    B: FnOnce(JIRSlice, JIRSlice, JIRSlice, U, U) -> (JIRInstruction, JIRSlice) + Copy,
    V: FnOnce(U, U) -> R + Copy,
    U: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U as FromBytes>::Bytes>>::Error: Debug,
    <U as FromBytes>::Bytes: Sized,
    R: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<R as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<R as FromBytes>::Bytes>>::Error: Debug,
    <R as FromBytes>::Bytes: Sized,
{
    for a in value_iter.clone() {
        for b in value_iter.clone() {
            test_instruction_generic(
                processor_spec,
                jit,
                context,
                instruction_builder,
                verify_op,
                reg_out,
                reg_a,
                reg_b,
                a,
                b,
            );
        }
    }
}

/// Test all combinations of values when iterating over the type, using the provided step size.
pub(super) fn test_multiple_binary_instruction_generic_different_sizes<T, U1, U2, R, B, V>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: B,
    verify_op: V,
    value_iter1: impl IntoIterator<Item = U1>,
    value_iter2: impl IntoIterator<Item = U2> + Clone,
    reg_out: &str,
    reg_a: &str,
    reg_b: &str,
) where
    T: MogiJit,
    B: FnOnce(JIRSlice, JIRSlice, JIRSlice, U1, U2) -> (JIRInstruction, JIRSlice) + Copy,
    V: FnOnce(U1, U2) -> R + Copy,
    U1: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U1 as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U1 as FromBytes>::Bytes>>::Error: Debug,
    <U1 as FromBytes>::Bytes: Sized,
    U2: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U2 as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U2 as FromBytes>::Bytes>>::Error: Debug,
    <U2 as FromBytes>::Bytes: Sized,
    R: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<R as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<R as FromBytes>::Bytes>>::Error: Debug,
    <R as FromBytes>::Bytes: Sized,
{
    for a in value_iter1 {
        for b in value_iter2.clone() {
            test_instruction_generic(
                processor_spec,
                jit,
                context,
                instruction_builder,
                verify_op,
                reg_out,
                reg_a,
                reg_b,
                a,
                b,
            );
        }
    }
}

pub(super) fn test_unary_instruction_generic<T, U, R>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: impl FnOnce(JIRSlice, JIRSlice, U) -> (JIRInstruction, JIRSlice),
    verify_op: impl FnOnce(U) -> R,
    reg_out: &str,
    reg_a: &str,
    a: U,
) where
    T: MogiJit,
    U: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U as FromBytes>::Bytes>>::Error: Debug,
    <U as FromBytes>::Bytes: Sized,
    R: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<R as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<R as FromBytes>::Bytes>>::Error: Debug,
    <R as FromBytes>::Bytes: Sized,
{
    let result = get_reg_slice(&processor_spec, reg_out);
    let slice_a = get_reg_slice(&processor_spec, reg_a);
    set_slice(context, slice_a, a);

    let (instruction, result) = instruction_builder(result, slice_a, a);
    let instructions = vec![
        //
        instruction,
    ];
    exec_instructions(jit, context, instructions);
    let actual: R = get_slice_value(context, &processor_spec, result);

    let expected = verify_op(a);
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}", actual, expected, a);

    // Check that r8 has not been touched
    let a_value: U = get_slice_value(context, &processor_spec, slice_a);
    if result != slice_a {
        assert_eq!(a_value, a);
    }
}

/// Test all combinations of values when iterating over the type, using the provided step size.
pub(super) fn test_multiple_unary_instruction_generic<T, U, R, B, V>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: B,
    verify_op: V,
    value_iter: impl IntoIterator<Item = U> + Clone,
    reg_out: &str,
    reg_a: &str,
) where
    T: MogiJit,
    B: FnOnce(JIRSlice, JIRSlice, U) -> (JIRInstruction, JIRSlice) + Copy,
    V: FnOnce(U) -> R + Copy,
    U: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U as FromBytes>::Bytes>>::Error: Debug,
    <U as FromBytes>::Bytes: Sized,
    R: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<R as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<R as FromBytes>::Bytes>>::Error: Debug,
    <R as FromBytes>::Bytes: Sized,
{
    for a in value_iter.clone() {
        for b in value_iter.clone() {
            test_unary_instruction_generic(
                processor_spec,
                jit,
                context,
                instruction_builder,
                verify_op,
                reg_out,
                reg_a,
                a,
            );
        }
    }
}
