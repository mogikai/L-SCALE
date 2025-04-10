use mogi_sleigh::jinst;

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value,
    set_slice,
    setup,
};

#[test]
fn test_ftof_f32_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
        f32::MAX as f64,
        f64::MAX,
    ];

    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for i in input.iter() {
        let i: f32 = *i as f32;

        set_slice(&mut *context, xmm1_da, i);

        let instructions = vec![jinst!(xmm0_qa = float2float(xmm1_da))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        assert_eq!(xmm0_qa_v, i as f64);
        assert_eq!(xmm1_da_v, i);
    }
}

#[test]
fn test_ftof_f64_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    let input = [
        4.194686791546667,
        -1.511787939180179,
        0.8031380809332509,
        -2.6176644319338926,
        -0.9862877822256095,
        f32::MAX as f64,
        f64::MAX,
    ];

    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for i in input.iter() {
        let i: f64 = *i as f64;

        set_slice(&mut *context, xmm1_qa, i);

        let instructions = vec![jinst!(xmm0_da = float2float(xmm1_qa))];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_qa_v: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        assert_eq!(xmm0_da_v, i as f32);
        assert_eq!(xmm1_qa_v, i);
    }
}
