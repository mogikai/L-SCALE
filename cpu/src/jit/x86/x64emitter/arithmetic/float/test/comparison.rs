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
fn test_fless_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f< input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fless_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f< input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f<= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f<= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f> input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f> input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f>= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f>= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f!= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a == input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f== input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f32_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 4).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f!= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f64_const_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        let input_a_jirval: JIRValue = JIRConst::new(input_a.to_bits().into(), 8).into();
        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        let instructions = vec![
            jinst!(al = input_a_jirval f!= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fless_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f< xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fless_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f< xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f<= xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f<= xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f> xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f> xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f>= xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f>= xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a == input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f== xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a == input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f== xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f32_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");
    let xmm1_da = get_reg_slice(&processor_spec, "XMM1_Da");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_da, input_a.to_bits());
        set_slice(&mut *context, xmm1_da, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f>= xmm1_da)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let xmm1_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm1_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(xmm1_da_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f64_reg_reg() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");
    let xmm1_qa = get_reg_slice(&processor_spec, "XMM1_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());
        set_slice(&mut *context, xmm1_qa, input_b.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f!= xmm1_qa)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let xmm1_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm1_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(xmm1_qa_val, input_b);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fless_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f< input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fless_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a < input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f< input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f<= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_flesseq_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a <= input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f<= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f> input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreater_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a > input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f> input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f>= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fgreatereq_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a >= input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f>= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a == input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f== input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_feq_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a == input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f== input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f32_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f32, f32); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_da = get_reg_slice(&processor_spec, "XMM0_Da");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 4).into();

        set_slice(&mut *context, xmm0_da, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_da f!= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_da_val: f32 = get_slice_value(&*context, &processor_spec, xmm0_da);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_da_val, input_a);
        assert_eq!(al_val, expected);
    }
}

#[test]
fn test_fneq_f64_reg_const() {
    let (mut jit, processor_spec, mut context) = setup(false);

    let input: [(f64, f64); 6] = [
        (4.271014789986124, -1.5696131304099694),
        (1.029667244040569, -2.818184468432512),
        (1.2533093156926292, 1.138223843381983),
        (0.8323463569173564, 0.31317149595321947),
        (0.8323463569173564, 0.8323463569173564),
        (-3.9582777199417762, 3.250670441798473),
    ];

    let al = get_reg_slice(&processor_spec, "AL");
    let xmm0_qa = get_reg_slice(&processor_spec, "XMM0_Qa");

    for (input_a, input_b) in input {
        let expected = match input_a != input_b {
            true => 1,
            false => 0,
        };

        let input_b_jirval: JIRValue = JIRConst::new(input_b.to_bits().into(), 8).into();

        set_slice(&mut *context, xmm0_qa, input_a.to_bits());

        let instructions = vec![
            jinst!(al = xmm0_qa f!= input_b_jirval)
        ];

        exec_instructions(&mut jit, &mut context, instructions);

        let xmm0_qa_val: f64 = get_slice_value(&*context, &processor_spec, xmm0_qa);
        let al_val: i8 = get_slice_value(&*context, &processor_spec, al);
        assert_eq!(xmm0_qa_val, input_a);
        assert_eq!(al_val, expected);
    }
}
