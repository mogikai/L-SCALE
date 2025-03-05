use crate::jit::x86::test::{
    setup,
    test_multiple_binary_instruction_generic_different_sizes,
};
use mogi_sleigh::jinst;

const TEST_STEP_64BIT: usize = 1 << 58;
const TEST_STEP_32BIT: usize = 1 << 26;
const TEST_STEP_16BIT: usize = 1 << 10;

#[test]
fn test_sborrow_slice_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_sborrow_slice_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        "AL",
        "RBX",
        "RCX",
    );
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_sborrow_const_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        (i64::MIN..i64::MAX).step_by(TEST_STEP_64BIT),
        "AL",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_sborrow_slice_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_sborrow_slice_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        "AL",
        "EBX",
        "ECX",
    );
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_sborrow_const_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        (i32::MIN..i32::MAX).step_by(TEST_STEP_32BIT),
        "AL",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_sborrow_slice_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_sborrow_slice_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        "AL",
        "BX",
        "CX",
    );
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_sborrow_const_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        (i16::MIN..i16::MAX).step_by(TEST_STEP_16BIT),
        "AL",
        "BX",
        "CX",
    );
}

#[test]
fn test_sborrow_slice_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        i8::MIN..i8::MAX,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_sborrow_slice_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        i8::MIN..i8::MAX,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        i8::MIN..i8::MAX,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_sborrow_const_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = sborrow(a, b)), result),
        |a, b| a.overflowing_sub(b).1 as u8,
        i8::MIN..i8::MAX,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}
