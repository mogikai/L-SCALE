use crate::jit::api::MogiJit;
use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_mem::PageOffset;
use mogi_mem::physical::page::PageData;
use mogi_sleigh::jinst;
use mogi_sleigh::spec::SpaceId;
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};

#[test]
fn test_load_tlb_hit() {
    let (mut jit, spec, mut context) = setup(false);
    jit.register_tlb_read_miss_callback(|context, evaddr, size, space_id| unsafe {
        // TODO: There is an issue with his panic. Somehow it triggers a null access
        panic!("This callback should not be called");
    })
    .unwrap();
    let reg_out = "RAX";
    let addr_reg = "RBX";
    let a = 0x800fafafu64;
    let value = 0x1234567890abcdefu64;
    let result = get_reg_slice(&spec, reg_out);
    let addr_slice = get_reg_slice(&spec, addr_reg);
    set_slice(&mut *context, addr_slice, a);

    let mut page = Box::new(PageData::zero());
    page.data[0xfaf..0xfaf + 8].copy_from_slice(&value.to_le_bytes());

    let addr = EVAddr::from(a);
    unsafe {
        context
            .tlb
            .insert_read(addr, HVAddr::new_unchecked(page.as_mut() as *mut _));
    }

    let instructions = vec![
        //
        jinst!(result = *[{SpaceId::from(0)}]addr_slice),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let actual: u64 = get_slice_value(context.as_ref(), &spec, result);

    let expected = value;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Address: {:#x}", actual, expected, a);
}

#[test]
fn test_load_tlb_miss() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "RAX";
    let addr_reg = "RBX";
    let value = 0x1234567890abcdefu64;
    let result = get_reg_slice(&spec, reg_out);
    let addr_slice = get_reg_slice(&spec, addr_reg);

    let mut page = Box::new(PageData::zero());
    page.data[0xfaf..0xfaf + 8].copy_from_slice(&value.to_le_bytes());

    let page_addr = unsafe { HVAddr::new_unchecked(page.as_mut() as *mut _) };
    // for this test we simply return a fixed address.
    jit.register_tlb_read_miss_callback(|context, evaddr, size, space_id| unsafe {
        Ok(page_addr.byte_offset(evaddr.page_offset() as isize))
    })
    .unwrap();

    let addr = EVAddr::<u8>::from(0x800fafafu64);
    set_slice(&mut *context, addr_slice, u64::from(addr));

    let instructions = vec![
        //
        jinst!(result = *[{SpaceId::from(0)}]addr_slice),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let actual: u64 = get_slice_value(context.as_ref(), &spec, result);

    let expected = value;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}", actual, expected, u64::from(addr));
}
