use std::slice;
use itertools::Itertools;

macro_rules! slice_op {
    ($out:expr, $out_len:expr, $in_a:expr, $in_a_len:expr, $in_b:expr, $in_b_len:expr, $op_type:ty, $op:expr) => {{
        let len = $out_len;
        let input = if $out as *const u8 == $in_a {
            $in_b
        } else if $out as *const u8 == $in_b {
            $in_a
        } else {
            // Copy in_a into output
            unsafe { $out.copy_from($in_a, $in_a_len) };
            $in_b
        };
        let output: &mut [$op_type] = unsafe { slice::from_raw_parts_mut($out as *mut $op_type, len) };
        let input: &[$op_type] = unsafe { slice::from_raw_parts(input as *const $op_type, len) };
        for (out, in_) in output.iter_mut().zip(input.iter()) {
            *out = $op(*out, *in_);
        }
    }};
}

macro_rules! slice_in_out_op {
    ($out:expr, $out_len:expr, $input:expr, $input_len:expr, $op_type:ty, $op:expr) => {{
        let out = unsafe { slice::from_raw_parts_mut($out as *mut $op_type, $out_len / size_of::<$op_type>()) };
        let input = unsafe { slice::from_raw_parts($input as *const $op_type, $input_len / size_of::<$op_type>()) };
        for (out, in_) in out.iter_mut().zip(input.iter()) {
            *out = $op(*out, *in_);
        }
    }};
}

// See Figure in the Intel Manual Page 1518
macro_rules! horizontal_slice_op {
    ($out:expr, $out_len:expr, $in_a:expr, $in_a_len:expr, $in_b:expr, $in_b_len:expr, $op_type:ty, $op:expr) => {{
        let len = $out_len;
        let input = if $out as *const u8 == $in_a {
            $in_b
        } else if $out as *const u8 == $in_b {
            $in_a
        } else {
            // Copy in_a into output
            unsafe { $out.copy_from($in_a, $in_a_len) };
            $in_b
        };
        let output: &mut [$op_type] = unsafe { slice::from_raw_parts_mut($out as *mut $op_type, len) };
        let output2 = output.to_vec();
        let input: &[$op_type] = unsafe { slice::from_raw_parts(input as *const $op_type, len) };
        for (out, (in_a, in_b)) in output.iter_mut()
            .zip(output2.iter().chain(input.iter()).tuples()) {
            *out = $op(*in_a, *in_b);
        }
    }};
}

// See Figure in the Intel Manual Page 1518
macro_rules! horizontal_slice_in_out_op {
    ($out:expr, $out_len:expr, $input:expr, $input_len:expr, $op_type:ty, $op:expr) => {{
        let out = unsafe { slice::from_raw_parts_mut($out as *mut $op_type, $out_len / size_of::<$op_type>()) };
        let out2 = out.to_vec();
        let input = unsafe { slice::from_raw_parts($input as *const $op_type, $input_len / size_of::<$op_type>()) };
        for (out, (in_a, in_b)) in out.iter_mut()
            .zip(out2.iter().chain(input.to_vec().iter()).tuples()) {
            *out = $op(*in_a, *in_b);
    }
    }};
}

pub unsafe extern "sysv64" fn paddsw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    // FIXME: Theoretically it is possible that all three arguments overlap in different parts
    //        Catching all these special cases would make the normal path slow.
    //        maybe we will have to add another "overlapping" fast impl. That way we could
    //        differentiate these cases during compile time.
    debug_assert!(out_len == in_a_len);
    debug_assert!(in_a_len == in_b_len);
    slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        i16,
        i16::saturating_add
    );
}

pub unsafe extern "sysv64" fn paddsw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    slice_in_out_op!(out, out_len, input, input_len, i16, i16::saturating_add);
}

pub unsafe extern "sysv64" fn psubsw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        i16,
        i16::saturating_sub
    );
}

pub unsafe extern "sysv64" fn psubsw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    slice_in_out_op!(out, out_len, input, input_len, i16, i16::saturating_sub);
}

pub unsafe extern "sysv64" fn paddusw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        u16,
        u16::saturating_add
    );
}

pub unsafe extern "sysv64" fn paddusw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    slice_in_out_op!(out, out_len, input, input_len, u16, u16::saturating_add);
}

pub unsafe extern "sysv64" fn psubusw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        u16,
        u16::saturating_sub
    );
}

pub unsafe extern "sysv64" fn psubusw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    slice_in_out_op!(out, out_len, input, input_len, u16, u16::saturating_sub);
}

pub unsafe extern "sysv64" fn pavgw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    let avg = |a: u16, b: u16| {
        let sum = a.saturating_add(b.saturating_add(1));
        sum >> 1
    };
    slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        u16,
        avg
    );
}

pub unsafe extern "sysv64" fn pavgw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    let avg = |a: u16, b: u16| {
        let sum = a.saturating_add(b.saturating_add(1));
        sum >> 1
    };
    slice_in_out_op!(out, out_len, input, input_len, u16, avg);
}


pub unsafe extern "sysv64" fn phaddw_slice(
    out: *mut u8,
    out_len: usize,
    in_a: *const u8,
    in_a_len: usize,
    in_b: *const u8,
    in_b_len: usize,
) {
    horizontal_slice_op!(
        out,
        out_len,
        in_a,
        in_a_len,
        in_b,
        in_b_len,
        i16,
        i16::saturating_add
    );
}

pub unsafe extern "sysv64" fn phaddw_slice_in_out(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    horizontal_slice_in_out_op!(out, out_len, input, input_len, i16, i16::saturating_add);
}
