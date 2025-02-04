use crate::jit::x86::test::{
    setup,
    test_multiple_binary_instruction_generic,
};
use mogi_sleigh::jinst;
use std::ops::Not;
fn neg<T: Not<Output = T>>(
    a: T,
    b: T,
) -> T {
    a.not()
}

#[test]
fn test_negate_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, _| (jinst!(result = ~a), result),
        neg,
        0u8..20,
        "AL",
        "R8B",
        "R9B",
    );
}

#[test]
fn test_negate_const_byte() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, _| (jinst!(result = ~a), result),
        neg,
        0u8..20,
        "AL",
        "R8B",
        "R9B",
    );
}

#[test]
fn test_negate_qword() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, _| (jinst!(result = ~a), result),
        neg,
        0u64..20,
        "RAX",
        "R8",
        "R9",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, _| (jinst!(result = ~a), result),
        neg,
        0u64..20,
        "RAX",
        "R8",
        "R9",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, _| (jinst!(result = ~a), result),
        neg,
        (u64::MAX - 20)..u64::MAX,
        "RAX",
        "R8",
        "R9",
    );
    test_multiple_binary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, _| (jinst!(result = ~a), result),
        neg,
        (u64::MAX - 20)..u64::MAX,
        "RAX",
        "R8",
        "R9",
    );
}
