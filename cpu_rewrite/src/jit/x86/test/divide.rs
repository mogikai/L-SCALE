use crate::jit::x86::test::{
    setup,
    test_multiple_instruction_generic,
};
use mogi_sleigh::jinst;

#[test]
fn test_divide_const_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_divide_const_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_divide_const_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_divide_const_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_slice_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_divide_slice_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_divide_slice_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_divide_slice_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_slice_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_divide_slice_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_divide_slice_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_divide_slice_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_const_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_divide_const_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_divide_const_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_divide_const_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a / b), result),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_const_slice_64_input_equal_output() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = a / b), b),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a / b), a),
        |a, b| a.wrapping_div(b),
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_signed_const_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a s/ b), result),
        |a, b| a.wrapping_div(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_divide_signed_slice_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a s/ b), result),
        |a, b| a.wrapping_div(b),
        (i64::MIN..i64::MAX)
            .step_by(1 << (i64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
        "RCX",
    );
}
