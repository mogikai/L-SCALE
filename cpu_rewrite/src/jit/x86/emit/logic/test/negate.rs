use crate::jit::x86::test::{
    setup,
    test_multiple_instruction_generic,
};
use mogi_sleigh::jinst;

fn neg(
    a: u8,
    b: u8,
) -> u8 {
    (!(a != 0)) as u8
}

#[test]
fn test_negate_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, _| (jinst!(result = !a), result),
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
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, _, _, a, _| (jinst!(result = !{ (a != 0) as u8 }), result),
        neg,
        19..20,
        "AL",
        "R8B",
        "R9B",
    );
}
