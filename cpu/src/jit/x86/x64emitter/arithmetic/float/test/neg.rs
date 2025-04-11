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
fn test_fneg_f32_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for input in input.iter() {
        let input_f32 = *input as f32;
        let output_f32 = -input_f32;

        set_slice(&mut *context, xmm1_da, input_f32.to_bits());

        let instructions = vec![jinst!(xmm0_da = fneg xmm1_da)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm0_da_v, output_f32);
        assert_eq!(xmm1_da_v, input_f32);
    }
}

#[test]
fn test_fneg_f64_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for input in input.iter() {
        let input_f64 = *input as f64;
        let output_f64 = -input_f64;

        set_slice(&mut *context, xmm1_qa, input_f64.to_bits());

        let instructions = vec![jinst!(xmm0_qa = fneg xmm1_qa)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm0_qa_v, output_f64);
        assert_eq!(xmm1_qa_v, input_f64);
    }
}

#[test]
fn test_fneg_f32_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for input in input.iter() {
        let input_f32 = *input as f32;
        let input_jirval: JIRValue = JIRConst::new(input_f32.to_bits().into(), 4).into();
        let output_f32 = -input_f32;

        let instructions = vec![jinst!(xmm0_da = fneg input_jirval)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        assert_eq!(xmm0_da_v, output_f32);
    }
}

#[test]
fn test_fneg_f64_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for input in input.iter() {
        let input_f64 = *input as f64;
        let input_jirval: JIRValue = JIRConst::new(input_f64.to_bits().into(), 8).into();
        let output_f64 = -input_f64;

        let instructions = vec![jinst!(xmm0_qa = fneg input_jirval)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        assert_eq!(xmm0_qa_v, output_f64);
    }
}
