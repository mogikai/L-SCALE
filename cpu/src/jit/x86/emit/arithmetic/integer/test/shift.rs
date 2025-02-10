use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    get_slice_value_by_name,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;

#[test]
fn test_shift_left_slice_slice() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let rax_slice = get_reg_slice(&processor_spec, "RAX");
    let rbx_slice = get_reg_slice(&processor_spec, "RBX");
    let rcx_slice = get_reg_slice(&processor_spec, "CL");
    set_slice(&mut *context, rax_slice, 0x41414141u64);
    set_slice(&mut *context, rbx_slice, 1u64);
    set_slice(&mut *context, rcx_slice, 6u8);

    let instructions = vec![
        //
        jinst!(rax_slice = rbx_slice << rcx_slice),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);

    let rax_value: u64 = get_slice_value(&*context, &processor_spec, rax_slice);
    assert_eq!(rax_value, 1 << 6);
}
