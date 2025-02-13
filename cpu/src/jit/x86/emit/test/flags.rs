use crate::jit::x86::test::{
    setup,
    test_multiple_binary_instruction_generic_different_sizes,
};
use mogi_sleigh::jinst;

#[test]
fn test_carry_slice_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = carry(a, b)), result),
        |a, b| a.overflowing_add(b).1 as u8,
        u8::MIN..u8::MAX,
        u8::MIN..u8::MAX,
        "AL",
        "BL",
        "CL",
    );
}

#[test]
fn test_scarry_slice_slice_8bit() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_binary_instruction_generic_different_sizes(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = scarry(a, b)), result),
        |a, b| a.overflowing_add(b).1 as u8,
        i8::MIN..i8::MAX,
        i8::MIN..i8::MAX,
        "AL",
        "BL",
        "CL",
    );
}
