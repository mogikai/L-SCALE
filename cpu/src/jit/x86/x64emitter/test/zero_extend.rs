use crate::jit::x86::test::{
    setup,
    test_multiple_unary_instruction_generic,
};
use mogi_sleigh::jinst;

const TEST_STEP16: usize = 1 << 10;
const TEST_STEP32: usize = 1 << 26;

#[test]
fn test_zero_extend_slice8_slice64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = zext(a)), result),
        |a| u64::from(a),
        u8::MIN..u8::MAX,
        "RAX",
        "BL",
    );
}

#[test]
fn test_zero_extend_slice16_slice64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = zext(a)), result),
        |a| u64::from(a),
        (u16::MIN..u16::MAX).step_by(TEST_STEP16),
        "RAX",
        "BX",
    );
}

#[test]
fn test_zero_extend_slice32_slice64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_unary_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _| (jinst!(result = zext(a)), result),
        |a| u64::from(a),
        (u32::MIN..u32::MAX).step_by(TEST_STEP32),
        "RAX",
        "EBX",
    );
}
