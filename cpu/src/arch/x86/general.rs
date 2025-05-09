use mogi_utilities::overlap::overlaps;
use num_traits::PrimInt;
use std::mem::{
    MaybeUninit,
    swap,
};
use std::slice;

pub extern "sysv64" fn swap_bytes<T: PrimInt>(a: T) -> T {
    a.swap_bytes()
}

#[inline(always)]
pub unsafe extern "sysv64" fn swap_bytes_non_overlapping(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    let out = unsafe { slice::from_raw_parts_mut(out, out_len) };
    let input = unsafe { slice::from_raw_parts(input, input_len) };
    debug_assert_eq!(out.len(), input.len());
    // TODO: This loop seems to produce vectorized instructions. Maybe this makes more sense for large inputs?
    // out.iter_mut()
    //     .zip(input.iter().rev())
    //     .for_each(|(a, b)| *a = *b);
    for i in 0..out.len() {
        out[i] = input[input.len() - i - 1];
    }
}

#[inline(always)]
pub unsafe extern "sysv64" fn swap_bytes_slice_in_out(
    out: *mut u8,
    out_len: usize,
) {
    let out = unsafe { slice::from_raw_parts_mut(out, out_len) };
    out.reverse();
}
