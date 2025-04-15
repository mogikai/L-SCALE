use crate::jit::x86::test::{
    setup,
    test_multiple_binary_instruction_generic_different_sizes,
};
use mogi_sleigh::jinst;
use std::ops::Shl;

#[test]
fn test_shift_left_slice_slice_128bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u128::MIN..u128::MIN + 100).chain(u128::MAX - 100..u128::MAX),
        0..64u8,
        "QAX",
        "QBX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u64::MIN..u64::MAX).step_by(1 << 56),
        0..64u8,
        "RAX",
        "RBX",
        "CL",
    );
}

#[test]
fn test_shift_left_output_same_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u64::MIN..u64::MAX).step_by(1 << 56),
        0..64u8,
        "RAX",
        "RAX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_const_128bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u128::MIN..u128::MIN + 100).chain(u128::MAX - 100..u128::MAX),
        0..64u8,
        "QAX",
        "QBX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_const_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u64::MIN..u64::MAX).step_by(1 << 56),
        0..64u8,
        "RAX",
        "RBX",
        "CL",
    );
}

#[test]
fn test_shift_left_const_slice_128bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u128::MIN..u128::MIN + 100).chain(u128::MAX - 100..u128::MAX),
        0..64u8,
        "QAX",
        "QBX",
        "CL",
    );
}

#[test]
fn test_shift_left_const_slice_64bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u64::MIN..u64::MAX).step_by(1 << 56),
        0..64u8,
        "RAX",
        "RBX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u32::MIN..u32::MAX).step_by(1 << 24),
        0..32u8,
        "EAX",
        "EBX",
        "CL",
    );
}

#[test]
fn test_shift_left_output_same_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u32::MIN..u32::MAX).step_by(1 << 24),
        0..32u8,
        "EAX",
        "EAX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_const_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u32::MIN..u32::MAX).step_by(1 << 24),
        0..32u8,
        "EAX",
        "EBX",
        "CL",
    );
}

#[test]
fn test_shift_left_const_slice_32bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u32::MIN..u32::MAX).step_by(1 << 24),
        0..32u8,
        "EAX",
        "EBX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u16::MIN..u16::MAX).step_by(1 << 12),
        0..16u8,
        "AX",
        "BX",
        "CL",
    );
}

#[test]
fn test_shift_left_output_same_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u16::MIN..u16::MAX).step_by(1 << 12),
        0..16u8,
        "AX",
        "AX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_const_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u16::MIN..u16::MAX).step_by(1 << 12),
        0..16u8,
        "AX",
        "BX",
        "CL",
    );
}

#[test]
fn test_shift_left_const_slice_16bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        (u16::MIN..u16::MAX).step_by(1 << 12),
        0..16u8,
        "AX",
        "BX",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        u8::MIN..u8::MAX,
        0..8u8,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_shift_left_output_same_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        u8::MIN..u8::MAX,
        0..8u8,
        "AL",
        "AL",
        "CL",
    );
}

#[test]
fn test_shift_left_slice_const_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        u8::MIN..u8::MAX,
        0..8u8,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_shift_left_const_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a << b), result),
        |a, b| a.shl(b),
        u8::MIN..u8::MAX,
        0..8u8,
        "AL",
        "BL",
        "CL",
    );
}
