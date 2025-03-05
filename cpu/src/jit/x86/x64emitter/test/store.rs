use crate::jit::api::{
    JITTaskRuntimeContext,
    MogiJit,
};
use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    set_slice,
    setup,
};
use crate::tlb::PageData;
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
fn test_store_tlb_hit() {
    let (mut jit, spec, mut context) = setup(false);
    jit.register_tlb_write_miss_callback(|context, evaddr, size, space_id| {
        panic!("This callback should not be called");
    })
    .unwrap();
    let value_reg = "RAX";
    let addr_reg = "RBX";
    let a = 0x800fafa0u64;
    let value = 0x1234567890abcdefu64;
    let value_slice = get_reg_slice(&spec, value_reg);
    let addr_slice = get_reg_slice(&spec, addr_reg);
    set_slice(&mut *context, addr_slice, a);
    set_slice(&mut *context, value_slice, value);

    let mut page = Box::new(PageData::zero());

    let addr = EVAddr::from(0x800fafafu64);
    let id = CustomOpId::try_from_index(5).unwrap();
    unsafe {
        context
            .tlb_mut()
            .insert_write(addr, HVAddr::new_unchecked(page.as_mut() as *mut _));
    }

    let instructions = vec![
        //
        jinst!(*[{SpaceId::from(0)}]addr_slice = value_slice),
    ];
    exec_instructions(&mut jit, &mut context, instructions);

    let actual = u64::from_le_bytes(page.data()[0xfa0..0xfa0 + 8].try_into().unwrap());

    let expected = value;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}", actual, expected, a);
}
