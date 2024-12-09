use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;

#[test]
fn test_complex() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_a = "EBX";
    let reg_b = "ECX";
    let a = 2348u32;
    let b = 35235u32;
    let result = get_reg_slice(&spec, reg_out);
    let slice_a = get_reg_slice(&spec, reg_a);
    let slice_b = get_reg_slice(&spec, reg_b);
    set_slice(&jit, &mut context, slice_a, a);
    set_slice(&jit, &mut context, slice_b, b);

    let instructions = vec![
        //
        jinst!(result = slice_a * slice_b),
        jinst!(slice_b = slice_a),
        jinst!(slice_b = slice_b - 43983),
        jinst!(result = result / slice_b),
        jinst!(result = result + 342),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let actual: u32 = get_slice_value(&jit, &context, &spec, result);

    let expected = (a * b) / (a.wrapping_sub(43983)) + 342;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}, {:#x}", actual, expected, a, b);
}
