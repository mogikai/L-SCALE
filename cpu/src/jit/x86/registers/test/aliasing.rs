use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;

#[ignore]
#[test]
fn aliasing_register_alloc_works() {
    let (mut jit, spec, mut context) = setup(true);
    let reg_rax = "RAX";
    let reg_rbx = "RBX";
    let reg_rcx = "RCX";
    let reg_ecx = "ECX";
    let rbx = 0xdeadbeefdeadbeefu64;
    let ecx = 0xc0ffeeu32;
    let slice_rax = get_reg_slice(&spec, reg_rax);
    let slice_rbx = get_reg_slice(&spec, reg_rbx);
    let slice_rcx = get_reg_slice(&spec, reg_rcx);
    let slice_ecx = get_reg_slice(&spec, reg_ecx);
    set_slice(&mut *context, slice_rbx, rbx);
    set_slice(&mut *context, slice_ecx, ecx);

    let instructions = [
        // extend ecx to rcx
        jinst!(slice_rcx = zext(slice_ecx)),
    ];
    exec_instructions(&mut jit, &mut context, instructions);
    let actual: u64 = get_slice_value(context.as_ref(), &spec, slice_rax);

    let expected = ecx as u64;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}.", actual, expected);
}
