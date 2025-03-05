use crate::jit::x86::test::{
    setup,
    test_instruction_generic,
    test_multiple_binary_instruction_generic,
};
use mogi_sleigh::jinst;

const TEST_STEP_64: usize = 1 << 58;
const TEST_STEP_32: usize = 1 << 26;
const TEST_STEP_16: usize = 1 << 10;

#[test]
fn test_signed_greater_than_equals_slice_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64),
        "AL",
        "RBX",
        "RCX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_greater_than_equals_const_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32),
        "AL",
        "EBX",
        "ECX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_greater_than_equals_const_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16),
        "AL",
        "BX",
        "CX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_greater_than_equals_const_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_greater_than_equals_slice_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_greater_than_equals_const_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s>= b), result),
        |a, b| (a >= b) as u8,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}
