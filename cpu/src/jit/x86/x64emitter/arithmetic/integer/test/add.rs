//! This module contains all the tests related to [X86MogiJit::emit_add](crate::jit::x86::X86MogiJit::emit_add)

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value_by_name,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;

#[test]
fn test_add_overflow_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let r8 = get_reg_slice(&processor_spec, "R8B");
    let r9 = get_reg_slice(&processor_spec, "R9B");
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&mut *context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "AL");

    set_slice(&mut *context, r8, 255u8);
    set_slice(&mut *context, r9, 5u8);

    let instructions = vec![
        //
        jinst!(result = r8 + r9),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let al: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "AL");
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let overflowed = 255u8.overflowing_add(5u8).0;
    assert_eq!(al, overflowed);

    // Assert that the upper bits are untouched
    let overflowed_32 = 0x41414141u32 & !0xff | overflowed as u32;
    assert_eq!(eax, overflowed_32);

    // Check that r8 and r9 have not been touched
    let r8: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8B");
    let r9: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R9B");
    assert_eq!(r8, 255u8);
    assert_eq!(r9, 5u8);
}

#[test]
fn test_add_overflow_slice_subslice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let r8w_slice = get_reg_slice(&processor_spec, "R8W");
    let r8d_slice = get_reg_slice(&processor_spec, "R8D");
    let result_slice = get_reg_slice(&processor_spec, "EAX");
    // set_slice(&mut *context, result_slice, 0x41414141u32);
    //
    // set_slice(&mut *context, r8w_slice, 0x1234_5678u32);
    //
    // todo!()
    // let instructions = vec![
    //     //
    //     jinst!(result_slice = r8w_slice + r8d_slice),
    // ];
    // exec_instructions(&mut jit, &mut context, &instructions);
    // let al: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "AL");
    // let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");
    //
    // // Assert the overflow is correct
    // let overflowed = 255u8.overflowing_add(5u8).0;
    // assert_eq!(al, overflowed);
    //
    // // Assert that the upper bits are untouched
    // let overflowed_32 = 0x41414141u32 & !0xff | overflowed as u32;
    // assert_eq!(eax, overflowed_32);
    //
    // // Check that r8 and r9 have not been touched
    // let r8: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8B");
    // let r9: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R9B");
    // assert_eq!(r8, 255u8);
    // assert_eq!(r9, 5u8);
}

#[test]
fn test_add_overflow_slice_const() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let r8 = get_reg_slice(&processor_spec, "R8B");
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&mut *context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "AL");

    set_slice(&mut *context, r8, 255u8);

    let instructions = vec![
        //
        jinst!(result = r8 + 5u8),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let al: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "AL");
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let overflowed = 255u8.overflowing_add(5u8).0;
    assert_eq!(al, overflowed);

    // Assert that the upper bits are untouched
    let overflowed_32 = 0x41414141u32 & !0xff | overflowed as u32;
    assert_eq!(eax, overflowed_32);

    // Check that r8 has not been touched
    let r8: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8B");
    assert_eq!(r8, 255u8);
}

#[test]
fn test_add_overflow_const_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let r9 = get_reg_slice(&processor_spec, "R9B");
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&mut *context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "AL");

    set_slice(&mut *context, r9, 5u8);

    let instructions = vec![
        //
        jinst!(result = 255u8 + r9),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let al: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "AL");
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let overflowed = 255u8.overflowing_add(5u8).0;
    assert_eq!(al, overflowed);

    // Assert that the upper bits are untouched
    let overflowed_32 = 0x41414141u32 & !0xff | overflowed as u32;
    assert_eq!(eax, overflowed_32);

    // Check that r9 has not been touched
    let r9: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "R9B");
    assert_eq!(r9, 5u8);
}

#[test]
fn test_add_overflow_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&mut *context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "AL");

    let instructions = vec![
        //
        jinst!(result = 255u8 + 5u8),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let al: u8 = get_slice_value_by_name(&jit, &context, &processor_spec, "AL");
    let eax: u32 = get_slice_value_by_name(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let overflowed = 255u8.overflowing_add(5u8).0;
    assert_eq!(al, overflowed);

    // Assert that the upper bits are untouched
    let overflowed_32 = 0x41414141u32 & !0xff | overflowed as u32;
    assert_eq!(eax, overflowed_32);
}

use crate::jit::x86::test::test_multiple_binary_instruction_generic;

#[test]
fn test_add_const_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u8::MIN..u8::MAX).step_by(1 << (u8::BITS - 4)),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_add_const_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u16::MIN..u16::MAX).step_by(1 << (u16::BITS - 4)),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_add_const_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u32::MIN..u32::MAX).step_by(1 << (u32::BITS - 4)),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_add_const_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_slice_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u8::MIN..u8::MAX).step_by(1 << (u8::BITS - 4)),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_add_slice_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u16::MIN..u16::MAX).step_by(1 << (u16::BITS - 4)),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_add_slice_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u32::MIN..u32::MAX).step_by(1 << (u32::BITS - 4)),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_add_slice_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_slice_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u8::MIN..u8::MAX).step_by(1 << (u8::BITS - 4)),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_add_slice_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u16::MIN..u16::MAX).step_by(1 << (u16::BITS - 4)),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_add_slice_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u32::MIN..u32::MAX).step_by(1 << (u32::BITS - 4)),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_add_slice_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_const_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u8::MIN..u8::MAX).step_by(1 << (u8::BITS - 4)),
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_add_const_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u16::MIN..u16::MAX).step_by(1 << (u16::BITS - 4)),
        "AX",
        "BX",
        "CX",
    );
}

#[test]
fn test_add_const_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u32::MIN..u32::MAX).step_by(1 << (u32::BITS - 4)),
        "EAX",
        "EBX",
        "ECX",
    );
}

#[test]
fn test_add_const_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, b, a, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_const_slice_64_input_equal_output() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = a + b), b),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a + b), a),
        |a, b| a.wrapping_add(b),
        (u64::MIN..u64::MAX).step_by(1 << (u64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_signed_const_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, b| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (i64::MIN..i64::MAX).step_by(1 << (i64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}

#[test]
fn test_add_signed_slice_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a + b), result),
        |a, b| a.wrapping_add(b),
        (i64::MIN..i64::MAX).step_by(1 << (i64::BITS - 4)),
        "RAX",
        "RBX",
        "RCX",
    );
}
