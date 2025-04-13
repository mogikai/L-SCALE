use mogi_sleigh::jinst;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRValue,
};

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    get_slice_value_by_name,
    set_slice,
    setup,
};

#[test]
fn test_nan_f32_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        1.7101771215486714,
        -0.4896431053493133,
        -f32::NAN,
        3.5444248677490133,
        f32::NAN,
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for input in input.iter() {
        let input_f32 = *input as f32;
        let output = input_f32.is_nan() as u8;

        set_slice(&mut *context, xmm1_da, input_f32.to_bits());

        let instructions = vec![jinst!(al = nan(xmm1_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_v: u8 = get_slice_value(&*context, &processor_spec, al);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(al_v, output);
        // this doesn't work for NaN :)
        // assert_eq!(xmm1_da_v, input_f32);
    }
}

#[test]
fn test_nan_f64_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        1.7101771215486714,
        -0.4896431053493133,
        -f64::NAN,
        3.5444248677490133,
        f64::NAN,
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for input in input.iter() {
        let input_f64 = *input as f64;
        let output = input_f64.is_nan() as u8;

        set_slice(&mut *context, xmm1_qa, input_f64.to_bits());

        let instructions = vec![jinst!(al = nan(xmm1_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_v: u8 = get_slice_value(&*context, &processor_spec, al);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(al_v, output);
        // this doesn't work for NaN :)
        // assert_eq!(xmm1_da_v, input_f32);
    }
}

#[test]
fn test_nan_f32_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        1.7101771215486714,
        -0.4896431053493133,
        -f32::NAN,
        3.5444248677490133,
        f32::NAN,
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for input in input.iter() {
        let input_f32 = *input as f32;
        let input_jirval: JIRValue = JIRConst::new(input_f32.to_bits().into(), 4).into();
        let output = input_f32.is_nan() as u8;

        let instructions = vec![jinst!(al = nan(input_jirval))];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_v: u8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_v, output);
    }
}

#[test]
fn test_nan_f64_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        1.7101771215486714,
        -0.4896431053493133,
        -f64::NAN,
        3.5444248677490133,
        f64::NAN,
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for input in input.iter() {
        let input_f64 = *input as f64;
        let input_jirval: JIRValue = JIRConst::new(input_f64.to_bits().into(), 8).into();
        let output = input_f64.is_nan() as u8;

        let instructions = vec![jinst!(al = nan(input_jirval))];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_v: u8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_v, output);
    }
}
