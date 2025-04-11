use mogi_sleigh::{jinst, spec::jir::values::{JIRConst, JIRValue}};

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    get_slice_value_by_name,
    set_slice,
    setup,
};

#[test]
fn test_ceil_f32_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [5.0, -1.0, 1.0, -2.0, 0.0];

    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 4).into();
        let exp: f32 = *exp as f32;

        let instructions = vec![jinst!(xmm1_da = ceil(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_ceil_f64_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [5.0, -1.0, 1.0, -2.0, 0.0];

    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 8).into();
        let exp: f64 = *exp as f64;

        let instructions = vec![jinst!(xmm1_qa = ceil(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm1_qa_v, exp);
    }
}

#[test]
fn test_ceil_f32_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [5.0, -1.0, 1.0, -2.0, 0.0];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let exp: f32 = *exp as f32;

        set_slice(&mut *context, xmm0_da, i.to_bits());

        let instructions = vec![jinst!(xmm1_da = ceil(xmm0_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm0_da_v, i);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_ceil_f64_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [5.0, -1.0, 1.0, -2.0, 0.0];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let exp: f64 = *exp as f64;

        set_slice(&mut *context, xmm0_qa, i.to_bits());

        let instructions = vec![jinst!(xmm1_qa = ceil(xmm0_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm0_qa_v, i);
        assert_eq!(xmm1_qa_v, exp);
    }
}

#[test]
fn test_floor_f32_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 0.0, -3.0, -1.0];

    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 4).into();
        let exp: f32 = *exp as f32;

        let instructions = vec![jinst!(xmm1_da = floor(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_floor_f64_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 0.0, -3.0, -1.0];

    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 8).into();
        let exp: f64 = *exp as f64;

        let instructions = vec![jinst!(xmm1_qa = ceil(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm1_qa_v, exp);
    }
}

#[test]
fn test_floor_f32_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 0.0, -3.0, -1.0];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let exp: f32 = *exp as f32;

        set_slice(&mut *context, xmm0_da, i.to_bits());

        let instructions = vec![jinst!(xmm1_da = floor(xmm0_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm0_da_v, i);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_floor_f64_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 0.0, -3.0, -1.0];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let exp: f64 = *exp as f64;

        set_slice(&mut *context, xmm0_qa, i.to_bits());

        let instructions = vec![jinst!(xmm1_qa = floor(xmm0_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm0_qa_v, i);
        assert_eq!(xmm1_qa_v, exp);
    }
}

#[test]
fn test_round_f32_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 1.0, -3.0, -1.0];

    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 4).into();
        let exp: f32 = *exp as f32;

        let instructions = vec![jinst!(xmm1_da = round(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_round_f64_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 1.0, -3.0, -1.0];

    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let i_jirvalue: JIRValue = JIRConst::new(i.to_bits().into(), 8).into();
        let exp: f64 = *exp as f64;

        let instructions = vec![jinst!(xmm1_qa = ceil(i_jirvalue))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm1_qa_v, exp);
    }
}

#[test]
fn test_round_f32_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 1.0, -3.0, -1.0];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let exp: f32 = *exp as f32;

        set_slice(&mut *context, xmm0_da, i.to_bits());

        let instructions = vec![jinst!(xmm1_da = round(xmm0_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm0_da_v, i);
        assert_eq!(xmm1_da_v, exp);
    }
}

#[test]
fn test_round_f64_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4.0, -2.0, 1.0, -3.0, -1.0];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let exp: f64 = *exp as f64;

        set_slice(&mut *context, xmm0_qa, i.to_bits());

        let instructions = vec![jinst!(xmm1_qa = round(xmm0_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm0_qa_v, i);
        assert_eq!(xmm1_qa_v, exp);
    }
}
