use crate::jit::x86::test::{
    setup,
    test_multiple_unary_instruction_generic,
    test_unary_instruction_generic,
};
use mogi_sleigh::jinst;

const TEST_STEP_64: usize = 1 << 58;
const TEST_STEP_32: usize = 1 << 26;
const TEST_STEP_16: usize = 1 << 10;

#[test]
fn test_lzcount_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u64,
        (u64::MIN..u64::MAX).step_by(TEST_STEP_64),
        "RAX",
        "RBX",
    );
}

#[test]
fn test_lzcount_slice_same_output_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u64,
        (u64::MIN..u64::MAX).step_by(TEST_STEP_64),
        "RAX",
        "RAX",
    );
}

#[test]
fn test_lzcount_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u64,
        (u64::MIN..u64::MAX).step_by(TEST_STEP_64),
        "RAX",
        "RBX",
    );
}

#[test]
fn test_lzcount_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros(),
        (u32::MIN..u32::MAX).step_by(TEST_STEP_32),
        "EAX",
        "EBX",
    );
}

#[test]
fn test_lzcount_slice_same_output_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros(),
        (u32::MIN..u32::MAX).step_by(TEST_STEP_32),
        "EAX",
        "EAX",
    );
}

#[test]
fn test_lzcount_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros(),
        (u32::MIN..u32::MAX).step_by(TEST_STEP_32),
        "EAX",
        "EBX",
    );
}

#[test]
fn test_lzcount_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u16,
        (u16::MIN..u16::MAX).step_by(TEST_STEP_16),
        "AX",
        "BX",
    );
}

#[test]
fn test_lzcount_slice_same_output_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u16,
        (u16::MIN..u16::MAX).step_by(TEST_STEP_16),
        "AX",
        "AX",
    );
}

#[test]
fn test_lzcount_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u16,
        (u16::MIN..u16::MAX).step_by(TEST_STEP_16),
        "AX",
        "BX",
    );
}

#[test]
fn test_lzcount_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u8,
        u8::MIN..u8::MAX,
        "AL",
        "BL",
    );
}

#[test]
fn test_lzcount_slice_same_output_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u8,
        u8::MIN..u8::MAX,
        "AL",
        "AL",
    );
}

#[test]
fn test_lzcount_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = lzcount(a)), result),
        |a| a.leading_zeros() as u8,
        u8::MIN..u8::MAX,
        "AL",
        "BL",
    );
}
