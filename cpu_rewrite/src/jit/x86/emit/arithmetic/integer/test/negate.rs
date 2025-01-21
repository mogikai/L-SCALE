use crate::jit::x86::test::{
    setup,
    test_multiple_unary_instruction_generic,
};
use bitvec::macros::internal::funty::Integral;
use mogi_sleigh::jinst;
use num_traits::WrappingNeg;

#[test]
fn test_negate_const_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = -a), result),
        |a| (a as i8).wrapping_neg() as u8,
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
    );
}

#[test]
fn test_negate_const_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = -a), result),
        |a| (a as i16).wrapping_neg() as u16,
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
    );
}

#[test]
fn test_negate_const_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = -a), result),
        |a| (a as i32).wrapping_neg() as u32,
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
    );
}

#[test]
fn test_negate_const_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, a| (jinst!(result = -a), result),
        |a| (a as i64).wrapping_neg() as u64,
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
    );
}

#[test]
fn test_negate_slice_8() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = -a), result),
        |a| (a as i8).wrapping_neg() as u8,
        (u8::MIN..u8::MAX)
            .step_by(1 << (u8::BITS - 4))
            .filter(|i| i != &0),
        "AL",
        "BL",
    );
}

#[test]
fn test_negate_slice_16() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = -a), result),
        |a| (a as i16).wrapping_neg() as u16,
        (u16::MIN..u16::MAX)
            .step_by(1 << (u16::BITS - 4))
            .filter(|i| i != &0),
        "AX",
        "BX",
    );
}

#[test]
fn test_negate_slice_32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = -a), result),
        |a| (a as i32).wrapping_neg() as u32,
        (u32::MIN..u32::MAX)
            .step_by(1 << (u32::BITS - 4))
            .filter(|i| i != &0),
        "EAX",
        "EBX",
    );
}

#[test]
fn test_negate_slice_64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = -a), result),
        |a| (a as i64).wrapping_neg() as u64,
        (u64::MIN..u64::MAX)
            .step_by(1 << (u64::BITS - 4))
            .filter(|i| i != &0),
        "RAX",
        "RBX",
    );
}
