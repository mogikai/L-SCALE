use crate::jit::x86::test::{
    setup,
    test_multiple_instruction_generic,
};
use mogi_sleigh::jinst;

fn xor(
    a: u8,
    b: u8,
) -> u8 {
    ((a != 0) ^ (b != 0)) as u8
}

#[test]
fn test_xor_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, b, _, _| (jinst!(result = a ^^ b), result),
        xor,
        0u8..20,
        "AL",
        "R8B",
        "R9B",
    );
}

#[test]
fn test_xor_slice_const_byte() {
    let (mut jit, processor_spec, mut context) = setup(false);
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |result, a, _, _, b| (jinst!(result = a ^^ { (b != 0) as u8 }), result),
        xor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, a, _, _, b| (jinst!(a = a ^^ { (b != 0) as u8 }), a),
        xor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
    test_multiple_instruction_generic(
        &processor_spec,
        &mut jit,
        &mut context,
        |_, _, b, a, _| (jinst!(b = { (a != 0) as u8 } ^^ b), b),
        xor,
        0..20,
        "AL",
        "R8B",
        "R9B",
    );
}
