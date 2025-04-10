use mogi_sleigh::jinst;

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};

#[test]
fn test_trunc_f32_i32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4, -1, 0, -2, 0];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let r8d = get_reg_slice(&processor_spec, "R8D");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let exp: i32 = *exp as i32;

        set_slice(&mut *context, xmm0_da, i.to_bits());

        let instructions = vec![jinst!(r8d = trunc(xmm0_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let r8d_v: i32 = get_slice_value(&*context, &processor_spec, r8d);
        assert_eq!(xmm0_da_v, i);
        assert_eq!(r8d_v, exp);
    }
}

#[test]
fn test_trunc_f32_i64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4, -1, 0, -2, 0];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let r8 = get_reg_slice(&processor_spec, "R8");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f32 = *i as f32;
        let exp: i64 = *exp as i64;

        set_slice(&mut *context, xmm0_da, i.to_bits());

        let instructions = vec![jinst!(r8 = trunc(xmm0_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let r8_v: i64 = get_slice_value(&*context, &processor_spec, r8);
        assert_eq!(xmm0_da_v, i);
        assert_eq!(r8_v, exp);
    }
}

#[test]
fn test_trunc_f64_i32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4, -1, 0, -2, 0];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let r8d = get_reg_slice(&processor_spec, "R8D");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let exp: i32 = *exp as i32;

        set_slice(&mut *context, xmm0_qa, i.to_bits());

        let instructions = vec![jinst!(r8d = trunc(xmm0_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let r8d_v: i32 = get_slice_value(&*context, &processor_spec, r8d);
        assert_eq!(xmm0_qa_v, i);
        assert_eq!(r8d_v, exp);
    }
}

#[test]
fn test_trunc_f64_i64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
    ];

    let expected = [4, -1, 0, -2, 0];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let r8 = get_reg_slice(&processor_spec, "R8");

    for (i, exp) in input.iter().zip(expected.iter()) {
        let i: f64 = *i as f64;
        let exp: i64 = *exp as i64;

        set_slice(&mut *context, xmm0_qa, i.to_bits());

        let instructions = vec![jinst!(r8 = trunc(xmm0_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let r8_v: i64 = get_slice_value(&*context, &processor_spec, r8);
        assert_eq!(xmm0_qa_v, i);
        assert_eq!(r8_v, exp);
    }
}
