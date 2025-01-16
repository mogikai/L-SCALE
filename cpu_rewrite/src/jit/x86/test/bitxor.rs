use crate::jit::x86::test::{
    setup,
    test_multiple_instruction_generic,
};
use mogi_sleigh::jinst;
use std::ops::BitXor;

#[test]
fn test_bitxor_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a ^ b), result),
        <u8 as BitXor<u8>>::bitxor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
}

#[test]
fn test_bitxor_slice_const_byte() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a ^ b), result),
        <u8 as BitXor<u8>>::bitxor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a ^ b), a),
        <u8 as BitXor<u8>>::bitxor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = a ^ b), b),
        <u8 as BitXor<u8>>::bitxor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
}

#[test]
fn test_bitxor_slice_const_qword() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a ^ b), result),
        <u64 as BitXor<u64>>::bitxor,
        0..20,
        "RAX",
        "R8",
        "R9",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a ^ b), a),
        <u64 as BitXor<u64>>::bitxor,
        0..20,
        "RAX",
        "R8",
        "R9",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = a ^ b), b),
        <u64 as BitXor<u64>>::bitxor,
        0..20,
        "RAX",
        "R8",
        "R9",
    );
}
