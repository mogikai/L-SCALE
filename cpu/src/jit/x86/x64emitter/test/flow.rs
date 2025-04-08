use crate::jit::api::{
    JITPathChoice,
    MogiJit,
};
use crate::jit::x86::X86MogiJit;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    make_basic_block_from_instructions,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use mogi_utilities::addresses::EVAddr;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

#[test]
fn test_goto_direct_if_true() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_addr = "RDX";
    let reg_a = "EBX";
    let a = 2348u32;
    let result = get_reg_slice(&spec, reg_out);
    let slice_a = get_reg_slice(&spec, reg_a);
    let slice_addr = get_reg_slice(&spec, reg_addr);
    set_slice(&mut *context, slice_a, a);
    set_slice(&mut *context, slice_addr, 0x1234u64);

    let instructions = vec![
        //
        jinst!(if slice_a goto slice_addr),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let actual: u32 = get_slice_value(context.as_ref(), &spec, result);

    let program_counter = jit.instruction_pointer(context.as_ref()).unwrap();
    assert_eq!(u64::from(program_counter), 0x1234u64)
}

#[test]
fn test_goto_direct_if_false() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_addr = "RDX";
    let reg_a = "EBX";
    let a = 0u32;
    let result = get_reg_slice(&spec, reg_out);
    let slice_a = get_reg_slice(&spec, reg_a);
    let slice_addr = get_reg_slice(&spec, reg_addr);
    set_slice(&mut *context, slice_a, a);
    set_slice(&mut *context, slice_addr, 0x1234u64);

    let instructions = vec![
        //
        jinst!(if slice_a goto slice_addr),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let actual: u32 = get_slice_value(context.as_ref(), &spec, result);

    let program_counter = jit.instruction_pointer(context.as_ref()).unwrap();
    assert_eq!(u64::from(program_counter), 0x800200u64)
}

#[test]
fn test_goto_direct_if_true_chained() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_addr = "RDX";
    let reg_cond = "EBX";
    let cond = 2348u32;
    let slice_result = get_reg_slice(&spec, reg_out);
    let slice_cond = get_reg_slice(&spec, reg_cond);
    let slice_addr = get_reg_slice(&spec, reg_addr);
    set_slice(&mut *context, slice_cond, cond);
    set_slice(&mut *context, slice_addr, 0x8000200u64);

    let end_block_addr = setup_blocks(&mut jit, &mut context, slice_cond, slice_addr, slice_result);

    let ip = jit.instruction_pointer(&context).unwrap();
    jit.execute(&mut context, ip).unwrap();

    let actual: u32 = get_slice_value(context.as_ref(), &spec, slice_result);

    assert_eq!(actual, 0x042u32);

    let program_counter = jit.instruction_pointer(context.as_ref()).unwrap();
    assert_eq!(program_counter, end_block_addr)
}

#[test]
fn test_goto_direct_if_false_chained() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_addr = "RDX";
    let reg_cond = "EBX";
    let cond = 0u32;
    let slice_result = get_reg_slice(&spec, reg_out);
    let slice_cond = get_reg_slice(&spec, reg_cond);
    let slice_addr = get_reg_slice(&spec, reg_addr);
    set_slice(&mut *context, slice_cond, cond);
    set_slice(&mut *context, slice_addr, 0x8000200u64);
    set_slice(&mut *context, slice_result, 0x0u32);

    let end_block_addr = setup_blocks(&mut jit, &mut context, slice_cond, slice_addr, slice_result);

    let ip = jit.instruction_pointer(&context).unwrap();
    jit.execute(&mut context, ip).unwrap();

    let actual: u32 = get_slice_value(context.as_ref(), &spec, slice_result);

    assert_eq!(actual, 0x041u32);

    let program_counter = jit.instruction_pointer(context.as_ref()).unwrap();
    assert_eq!(program_counter, end_block_addr)
}

fn setup_blocks(
    jit: &mut X86MogiJit,
    mut context: &mut Box<X86TaskContext>,
    slice_cond: JIRSlice,
    slice_addr: JIRSlice,
    slice_result: JIRSlice,
) -> EVAddr {
    let start_block_addr = EVAddr::from(0x8000000u64);
    let false_block_addr = EVAddr::from(0x8000100u64);
    let true_block_addr = EVAddr::from(0x8000200u64);
    let end_block_addr = EVAddr::from(0x8000300u64);

    jit.set_instruction_pointer(&mut context, start_block_addr)
        .unwrap();

    jit.compile_target_block(&make_basic_block_from_instructions(
        start_block_addr,
        false_block_addr,
        [jinst!(if slice_cond goto slice_addr)],
    ))
    .unwrap();

    jit.compile_target_block(&make_basic_block_from_instructions(
        false_block_addr,
        end_block_addr,
        [jinst!(slice_result = 0x41u32)],
    ))
    .unwrap();

    jit.compile_target_block(&make_basic_block_from_instructions(
        true_block_addr,
        end_block_addr,
        [jinst!(slice_result = 0x42u32)],
    ))
    .unwrap();

    jit.chain_blocks(start_block_addr, false_block_addr, JITPathChoice::Main)
        .unwrap();
    jit.chain_blocks(start_block_addr, true_block_addr, JITPathChoice::Alternate)
        .unwrap();
    end_block_addr
}
