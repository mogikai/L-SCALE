use mogi_sleigh::jinst;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRValue,
};

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value_by_name,
    set_slice,
    setup,
};

#[test]
fn test_fmul_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);
    for k in 0..(i64::BITS - 2) {
        let a_i64: i64 = 2i64.pow(k);
        let b_i64: i64 = 2i64.pow(k + 1);
        let a_f32: f32 = a_i64 as f32;
        let b_f32: f32 = b_i64 as f32;

        let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
        let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");
        let xmm2_da = get_reg_slice(&processor_spec, "XMM2_Da");

        set_slice(&mut *context, xmm0_da, a_f32.to_bits());
        set_slice(&mut *context, xmm1_da, b_f32.to_bits());

        let instructions = vec![jinst!(xmm2_da = xmm0_da f* xmm1_da)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_v: f32 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Da");
        let xmm1_da_v: f32 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM1_Da");
        let xmm2_da_v: f32 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM2_Da");
        assert_eq!(xmm0_da_v, a_f32);
        assert_eq!(xmm1_da_v, b_f32);
        assert_eq!(xmm2_da_v, a_f32 * b_f32);
    }
}

#[test]
fn test_fmul_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);
    for k in 0..(i64::BITS - 2) {
        let a_i64: i64 = 2i64.pow(k);
        let b_i64: i64 = 2i64.pow(k + 1);
        let a_f64: f64 = a_i64 as f64;
        let b_f64: f64 = b_i64 as f64;

        let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
        let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");
        let xmm2_qa = get_reg_slice(&processor_spec, "XMM2_Qa");

        set_slice(&mut *context, xmm0_qa, a_f64.to_bits());
        set_slice(&mut *context, xmm1_qa, b_f64.to_bits());

        let instructions = vec![jinst!(xmm2_qa = xmm0_qa f* xmm1_qa)];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_v: f64 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Qa");
        let xmm1_qa_v: f64 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM1_Qa");
        let xmm2_qa_v: f64 = get_slice_value_by_name(&jit, &context, &processor_spec, "XMM2_Qa");
        assert_eq!(xmm0_qa_v, a_f64);
        assert_eq!(xmm1_qa_v, b_f64);
        assert_eq!(xmm2_qa_v, a_f64 * b_f64);
    }
}
