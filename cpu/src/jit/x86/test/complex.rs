use crate::jit::api::{
    JITTaskRuntimeContext,
    MogiJit,
};
use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};
use mogi_sleigh::jinst;
use mogi_sleigh::spec::CustomOpId;
use mogi_sleigh::spec::jir::values::JIRValue;
use mogi_utilities::index_map::MapKey;

#[test]
fn test_complex() {
    let (mut jit, spec, mut context) = setup(false);
    let reg_out = "EAX";
    let reg_addr = "RDX";
    let reg_a = "EBX";
    let reg_b = "ECX";
    let a = 2348u32;
    let b = 35235u32;
    let result = get_reg_slice(&spec, reg_out);
    let slice_a = get_reg_slice(&spec, reg_a);
    let slice_b = get_reg_slice(&spec, reg_b);
    let slice_addr = get_reg_slice(&spec, reg_addr);
    set_slice(&mut *context, slice_a, a);
    set_slice(&mut *context, slice_b, b);
    set_slice(&mut *context, slice_addr, 0x1234u64);

    let id = CustomOpId::try_from_index(5).unwrap();

    jit.register_custom_op_impl(id, |context, args| {
        let JIRValue::Slice(input_a) = args.input_a? else {
            panic!();
        };
        let input_a = u32::from_le_bytes(context.slice(input_a)?.try_into().unwrap());
        let JIRValue::Slice(input_b) = args.input_b? else {
            panic!();
        };
        let input_b = u32::from_le_bytes(context.slice(input_b)?.try_into().unwrap());
        let output = context.slice_mut(args.output?)?;
        output.copy_from_slice(&input_a.wrapping_sub(input_b.isqrt()).to_le_bytes());
        Ok(())
    })
    .unwrap();

    let instructions = vec![
        //
        jinst!(result = slice_a * slice_b),
        jinst!(slice_b = slice_a),
        jinst!(slice_b = slice_b - 43983),
        jinst!(result = (*id)(slice_a, slice_b)),
        jinst!(result = result / slice_b),
        jinst!(result = result + 342),
        jinst!(goto slice_addr),
    ];
    exec_instructions(&mut jit, &mut context, &instructions);
    let actual: u32 = get_slice_value(context.as_ref(), &spec, result);

    let result = a * b;
    let b = a;
    let b = b.wrapping_sub(43983);
    let result = a.wrapping_sub(b.isqrt());
    let result = result / b;
    let result = result + 342;
    let expected = result;
    assert_eq!(actual, expected, "Got {:#x} but expected {:#x}. Operands: {:#x}, {:#x}", actual, expected, a, b);
    let program_counter = jit.instruction_pointer(context.as_ref()).unwrap();
    assert_eq!(u64::from(program_counter), 0x1234u64)
}
