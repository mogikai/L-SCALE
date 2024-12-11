mod add;
mod bitand;
mod complex;
mod divide;
mod multiply;

use crate::jit::api::{
    JITRuntimeContext,
    MogiJit,
};
use crate::jit::x86::X86MogiJit;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::jinst;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_utilities::addresses::EVAddr;
use num_traits::{
    FromBytes,
    ToBytes,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::{
    Debug,
    LowerHex,
};
use std::ops::BitAnd;
use std::path::PathBuf;
use std::rc::Rc;

fn setup(
    debug: bool
) -> (X86MogiJit, Rc<RefCell<ProcessorSpecification>>, Box<<X86MogiJit as MogiJit>::RuntimeContext>) {
    let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_path.push("resources/test/x86/x86-64.slaspec");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let mut compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let specification = Rc::new(RefCell::new(compiler.compile(spec_path.as_path()).unwrap()));
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let jit = X86MogiJit::new(specification.clone(), mmu.clone(), debug);
    let context = Box::new(jit.new_runtime_context());
    (jit, specification, context)
}

fn set_slice<T: JITRuntimeContext, V: ToBytes>(
    context: &mut T,
    slice: JIRSlice,
    value: V,
) {
    context
        .slice_mut(slice)
        .unwrap()
        .copy_from_slice(value.to_le_bytes().as_ref());
}

fn get_slice_value_by_name<T: MogiJit, V: FromBytes>(
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

fn get_slice_value<T: JITRuntimeContext, V: FromBytes>(
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

fn get_reg_slice(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    name: &str,
) -> JIRSlice {
    processor_spec
        .borrow()
        .varnode_slice_by_string(name)
        .unwrap()
}

fn exec_instructions<T: MogiJit>(
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instructions: &[JIRInstruction],
) {
    jit.new_block(EVAddr::null()).unwrap();
    for instruction in instructions {
        jit.emit_jinst(&instruction).unwrap();
    }
    jit.emit_flush_slices().unwrap();
    jit.finalize_block().unwrap();

    let block = jit
        .fetch_block(jit.instruction_pointer(&context).unwrap())
        .unwrap();
    let context = &mut *context as *mut _ as *mut c_void;
    jit.prelude().unwrap()(context, block);
}

fn test_instruction_generic<T, U>(
    processor_spec: &Rc<RefCell<ProcessorSpecification>>,
    jit: &mut T,
    context: &mut T::RuntimeContext,
    instruction_builder: impl FnOnce(JIRSlice, JIRSlice, JIRSlice, U, U) -> (JIRInstruction, JIRSlice),
    verify_op: impl FnOnce(U, U) -> U,
    reg_out: &str,
    reg_a: &str,
    reg_b: &str,
    a: U,
    b: U,
) where
    T: MogiJit,
    U: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U as FromBytes>::Bytes>>::Error: Debug,
    <U as FromBytes>::Bytes: Sized,
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
    exec_instructions(jit, context, &instructions);
    let actual: U = get_slice_value(context, &processor_spec, result);

    let expected = verify_op(a, b);
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}, {:#x}", actual, expected, a, b);

    // Check that r8 and r9 have not been touched
    let a_value: U = get_slice_value(context, &processor_spec, slice_a);
    let b_value: U = get_slice_value(context, &processor_spec, slice_b);
    if result != slice_a {
        assert_eq!(a_value, a);
    }
    if result != slice_b {
        assert_eq!(b_value, b);
    }
}

/// Test all combinations of values when iterating over the type, using the provided step size.
fn test_multiple_instruction_generic<T, U, B, V>(
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
    V: FnOnce(U, U) -> U + Copy,
    U: ToBytes + FromBytes + PartialEq + Debug + LowerHex + Copy,
    for<'a> &'a [u8]: TryInto<<U as FromBytes>::Bytes>,
    for<'a> <&'a [u8] as TryInto<<U as FromBytes>::Bytes>>::Error: Debug,
    <U as FromBytes>::Bytes: Sized,
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
