use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_mem::physical::page::PageData;
use mogi_mem::tlb::{
    PageIndex,
    PageTag,
};
use mogi_sleigh::jinst;
use mogi_sleigh::spec::{
    CustomOpId,
    SpaceId,
};
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use mogi_utilities::index_map::MapKey;

#[test]
fn test_load() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "RAX";
    let addr_reg = "RBX";
    let a = 0x800fafafu64;
    let result = get_reg_slice(&spec, reg_out);
    let addr_slice = get_reg_slice(&spec, addr_reg);
    set_slice(&mut *context, addr_slice, a);

    let addr = EVAddr::from(0x800fafafu64);
    let id = CustomOpId::try_from_index(5).unwrap();
    unsafe {
        context
            .tlb
            .insert_read(addr, HVAddr::new_unchecked(0x1234u64 as *mut PageData));
    }

    let instructions = vec![
        //
        jinst!(result = *[{SpaceId::from(0)}]addr_slice),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let actual: u64 = get_slice_value(context.as_ref(), &spec, result);

    let expected = 0;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}", actual, expected, a);
}
