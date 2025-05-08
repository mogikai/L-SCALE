use std::slice;

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
    let len = out_len;
    let input = if out as *const u8 == in_a {
        in_b
    } else if out as *const u8 == in_b {
        in_a
    } else {
        // Copy in_a into output
        unsafe { out.copy_from(in_a, in_a_len) };
        in_b
    };
    let output: &mut [i16] = unsafe { slice::from_raw_parts_mut(out as *mut i16, len) };
    let input: &[i16] = unsafe { slice::from_raw_parts(input as *const i16, len) };
    for (out, in_) in output.iter_mut().zip(input.iter()) {
        *out = out.saturating_add(*in_);
    }
}
