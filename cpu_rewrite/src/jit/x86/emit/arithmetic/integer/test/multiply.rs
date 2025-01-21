use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value_by_name,
    set_slice,
    setup,
    test_multiple_binary_instruction_generic,
};
use mogi_sleigh::jinst;

#[test]
fn test_multiply_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let r8 = get_reg_slice(&processor_spec, "R8D");
    let r9 = get_reg_slice(&processor_spec, "R9D");
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&mut *context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "EAX");

    set_slice(&mut *context, r8, 5894373u32);
    set_slice(&mut *context, r9, 6427245u32);

    let instructions = vec![
        //
        jinst!(result = r8 * r9),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let expected = 5894373u32.wrapping_mul(6427245u32);
    assert_eq!(eax, expected);

    // Check that r8 and r9 have not been touched
    let r8: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8D");
    let r9: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "R9D");
    assert_eq!(r8, 5894373u32);
    assert_eq!(r9, 6427245u32);
}

#[test]
fn test_multiply_signed_slice_slice_multiple() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        (i32::MIN..i32::MAX).step_by(1 << 26),
        "EAX",
        "EBX",
        "ECX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, b, _, _| (jinst!(a = a * b), a),
        |a, b| a.wrapping_mul(b),
        (i32::MIN..i32::MAX).step_by(1 << 26),
        "EAX",
        "EBX",
        "ECX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, b, _, _| (jinst!(b = a * b), b),
        |a, b| a.wrapping_mul(b),
        (i32::MIN..i32::MAX).step_by(1 << 26),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_multiply_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let result = get_reg_slice(&processor_spec, "EAX");

    let instructions = vec![
        //
        jinst!(result = 3848432476u32 * 687485432u32),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let expected = 3848432476u32.overflowing_mul(687485432u32).0;
    assert_eq!(eax, expected);
}

#[test]
fn test_multiply_signed_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        (i32::MIN..i32::MAX).step_by(1 << 26),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_multiply_slice_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        u8::MIN..u8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_multiply_slice_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        (u16::MIN..u16::MAX).step_by(1 << (u16::BITS - 4)),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_multiply_slice_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        (u32::MIN..u32::MAX).step_by(1 << (u32::BITS - 4)),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_multiply_slice_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a * b), result),
        |a, b| a.wrapping_mul(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_multiply_slice_const_input_equal_output() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a * b), a),
        |a, b| a.wrapping_mul(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = b * a), a),
        |a, b| a.wrapping_mul(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}
