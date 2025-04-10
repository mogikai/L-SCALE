use mogi_sleigh::jinst;

use crate::jit::x86::test::{
    exec_instructions,
    get_reg_slice,
    get_slice_value_by_name,
    setup,
};

#[test]
fn test_itof_const_32_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i32::BITS {
        let c_i32 = 1 << k;
        let c_f32 = c_i32 as f32;

        let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

        let instructions = vec![jinst!(xmm0_da = int2float(c_i32))];

        exec_instructions(&mut jit, &mut context, instructions);
        let xmm0_da_val: f32 = f32::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Da"));

        assert_eq!(xmm0_da_val, c_f32);
    }
}

#[test]
fn test_itof_const_64_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i64::BITS {
        let c_i64 = 1 << k;
        let c_f32 = c_i64 as f32;

        let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

        let instructions = vec![jinst!(xmm0_da = int2float(c_i64))];

        exec_instructions(&mut jit, &mut context, instructions);
        let xmm0_da_val: f32 = f32::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Da"));

        assert_eq!(xmm0_da_val, c_f32);
    }
}

#[test]
fn test_itof_const_32_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i32::BITS {
        let c_i32 = 1 << k;
        let c_f64 = c_i32 as f64;

        let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

        let instructions = vec![jinst!(xmm0_qa = int2float(c_i32))];

        exec_instructions(&mut jit, &mut context, instructions);
        let xmm0_qa_val: f64 = f64::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Qa"));

        assert_eq!(xmm0_qa_val, c_f64);
    }
}

#[test]
fn test_itof_const_64_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i64::BITS {
        let c_i64 = 1 << k;
        let c_f64 = c_i64 as f64;

        let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

        let instructions = vec![jinst!(xmm0_qa = int2float(c_i64))];

        exec_instructions(&mut jit, &mut context, instructions);
        let xmm0_qa_val: f64 = f64::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Qa"));

        assert_eq!(xmm0_qa_val, c_f64);
    }
}

#[test]
fn test_itof_reg_32_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i32::BITS {
        let c_i32 = 1 << k;
        let c_f32 = c_i32 as f32;

        let r8d = get_reg_slice(&processor_spec, "R8D");
        let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

        let instructions = vec![jinst!(r8d = c_i32), jinst!(xmm0_da = int2float(r8d))];

        exec_instructions(&mut jit, &mut context, instructions);
        let r8d_val: i32 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8D");
        let xmm0_da_val: f32 = f32::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Da"));

        assert_eq!(r8d_val, c_i32);
        assert_eq!(xmm0_da_val, c_f32);
    }
}

#[test]
fn test_itof_reg_64_f32() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i64::BITS {
        let c_i64 = 1 << k;
        let c_f32 = c_i64 as f32;

        let r8 = get_reg_slice(&processor_spec, "R8");
        let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

        let instructions = vec![jinst!(r8 = c_i64), jinst!(xmm0_da = int2float(r8))];

        exec_instructions(&mut jit, &mut context, instructions);
        let r8_val: i64 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8");
        let xmm0_da_val: f32 = f32::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Da"));

        assert_eq!(r8_val, c_i64);
        assert_eq!(xmm0_da_val, c_f32);
    }
}

#[test]
fn test_itof_reg_32_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i32::BITS {
        let c_i32 = 1 << k;
        let c_f64 = c_i32 as f64;

        let r8d = get_reg_slice(&processor_spec, "R8D");
        let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

        let instructions = vec![jinst!(r8d = c_i32), jinst!(xmm0_qa = int2float(r8d))];

        exec_instructions(&mut jit, &mut context, instructions);
        let r8d_val: i32 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8D");
        let xmm0_qa_val: f64 = f64::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Qa"));

        assert_eq!(r8d_val, c_i32);
        assert_eq!(xmm0_qa_val, c_f64);
    }
}

#[test]
fn test_itof_reg_64_f64() {
    let (mut jit, processor_spec, mut context) = setup(false);

    for k in 0..i64::BITS {
        let c_i64 = 1 << k;
        let c_f64 = c_i64 as f64;

        let r8 = get_reg_slice(&processor_spec, "R8");
        let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

        let instructions = vec![jinst!(r8 = c_i64), jinst!(xmm0_qa = int2float(r8))];

        exec_instructions(&mut jit, &mut context, instructions);
        let r8_val: i64 = get_slice_value_by_name(&jit, &context, &processor_spec, "R8");
        let xmm0_qa_val: f64 = f64::from_bits(get_slice_value_by_name(&jit, &context, &processor_spec, "XMM0_Qa"));

        assert_eq!(r8_val, c_i64);
        assert_eq!(xmm0_qa_val, c_f64);
    }
}
