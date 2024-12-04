use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;

#[test]
fn test_multiply_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup();
    let r8 = get_reg_slice(&processor_spec, "R8D");
    let r9 = get_reg_slice(&processor_spec, "R9D");
    let eax = get_reg_slice(&processor_spec, "EAX");
    set_slice(&jit, &mut context, eax, 0x41414141u32);
    let result = get_reg_slice(&processor_spec, "EAX");

    set_slice(&jit, &mut context, r8, 5894373u32);
    set_slice(&jit, &mut context, r9, 6427245u32);

    let instructions = vec![
        //
        jinst!(result = r8 * r9),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let eax: u32 = get_slice_value(&jit, &context, &processor_spec, "EAX");

    // Assert the overflow is correct
    let expected = 5894373u32.overflowing_mul(6427245u32).0;
    assert_eq!(eax, expected);

    // Check that r8 and r9 have not been touched
    let r8: u32 = get_slice_value(&jit, &context, &processor_spec, "R8D");
    let r9: u32 = get_slice_value(&jit, &context, &processor_spec, "R9D");
    assert_eq!(r8, 5894373u32);
    assert_eq!(r9, 6427245u32);
}
