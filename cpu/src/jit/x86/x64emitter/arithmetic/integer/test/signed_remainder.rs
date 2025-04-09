use crate::jit::x86::test::{
    setup,
    test_multiple_binary_instruction_generic,
};
use mogi_sleigh::jinst;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{
    EnvFilter,
    fmt,
};

#[test]
fn test_signed_remainder_const_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i8::MIN..i8::MAX)
            .step_by(1 << (i8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_remainder_const_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i16::MIN..i16::MAX)
            .step_by(1 << (i16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_remainder_const_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i32::MIN..i32::MAX)
            .step_by(1 << (i32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_remainder_const_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_remainder_slice_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i8::MIN..i8::MAX)
            .step_by(1 << (i8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_remainder_slice_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i16::MIN..i16::MAX)
            .step_by(1 << (i16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_remainder_slice_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i32::MIN..i32::MAX)
            .step_by(1 << (i32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_remainder_slice_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_remainder_slice_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i8::MIN..i8::MAX)
            .step_by(1 << (i8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_remainder_slice_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i16::MIN..i16::MAX)
            .step_by(1 << (i16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_remainder_slice_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i32::MIN..i32::MAX)
            .step_by(1 << (i32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_remainder_slice_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_remainder_const_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i8::MIN..i8::MAX)
            .step_by(1 << (i8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_signed_remainder_const_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i16::MIN..i16::MAX)
            .step_by(1 << (i16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_signed_remainder_const_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i32::MIN..i32::MAX)
            .step_by(1 << (i32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_signed_remainder_const_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a s% b), result),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_signed_remainder_const_slice_64_input_equal_output() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = a s% b), b),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a s% b), a),
        |a, b| a.wrapping_rem(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}
