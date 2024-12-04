mod add;

use crate::jit::api::MogiJit;
use crate::jit::x86::X86MogiJit;
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::compiler::builder::SleighCompilerBuilder;
use mogi_sleigh::preprocessor::SleighPreprocessorFactory;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::jcode::JIRInstruction;
use mogi_sleigh::spec::jir::values::JIRSlice;
use mogi_utilities::addresses::EVAddr;
use num_traits::{
    FromBytes,
    ToBytes,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Debug;
use std::path::PathBuf;
use std::rc::Rc;

fn setup() -> (X86MogiJit, Rc<RefCell<ProcessorSpecification>>, Box<<X86MogiJit as MogiJit>::RuntimeContext>) {
    let mut spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    spec_path.push("resources/test/x86/x86-64.slaspec");
    let spec_dir = spec_path.parent().unwrap();
    let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
    let mut compiler = SleighCompilerBuilder::new()
        .preprocessor_factory(factory)
        .build();
    let specification = Rc::new(RefCell::new(compiler.compile(spec_path.as_path()).unwrap()));
    let mmu = Rc::new(RefCell::new(MogiMMU::new()));
    let jit = X86MogiJit::new(specification.clone(), mmu.clone(), false);
    let context = Box::new(jit.new_runtime_context());
    (jit, specification, context)
}

fn set_slice<T: MogiJit, V: ToBytes>(
    jit: &T,
    context: &mut T::RuntimeContext,
    slice: JIRSlice,
    value: V,
) {
    jit.slice_mut(context, slice)
        .unwrap()
        .copy_from_slice(value.to_le_bytes().as_ref());
}

fn get_slice_value<T: MogiJit, V: FromBytes>(
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
    context: &mut Box<T::RuntimeContext>,
    instructions: &[JIRInstruction],
) {
    jit.new_block(EVAddr::null()).unwrap();
    for instruction in instructions {
        jit.emit_jinst(&instruction).unwrap();
    }
    jit.emit_flush_varnodes().unwrap();
    jit.finalize_block().unwrap();

    let block = jit
        .fetch_block(jit.instruction_pointer(&context).unwrap())
        .unwrap();
    let context = &mut **context as *mut _ as *mut c_void;
    jit.prelude().unwrap()(context, block);
}
