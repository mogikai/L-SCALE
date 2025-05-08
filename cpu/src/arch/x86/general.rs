use mogi_utilities::overlap::overlaps;
use num_traits::PrimInt;
use std::mem::MaybeUninit;
use std::slice;

pub extern "sysv64" fn swap_bytes<T: PrimInt>(a: T) -> T {
    // TODO: This only works for 64 bit swaps. We need a way to implement this for other slice sizes as well
    //       One option would be to pass the size as an extra parameter. However, this would require loading
    //       the size for each slice into a register. If possible we should avoid this.
    //       Another could be to have a separate function for each slice size. This is feasible since we are
    //       dealing with sizes one through 8 (bytes).
    //       Another possible optimization could be to compile the functions without a function prologue and epilogue
    //       with a custom calling convention. That way we could avoid clobbering as many registers as we currently do.
    a.swap_bytes()
}

/// # SAFETY
/// - `out` must be a pointer to a valid region of memory containing `out_len` bytes.
/// - `input` must be a pointer to a valid region of memory containing `input_len` bytes.
/// - `out_len` and `input_len` must be equal.
/// - The memory of input may overlap with output.
pub unsafe extern "sysv64" fn swap_bytes_slice(
    out: *mut u8,
    out_len: usize,
    input: *const u8,
    input_len: usize,
) {
    debug_assert_eq!(out_len, input_len);
    let output = unsafe { slice::from_raw_parts_mut(out, out_len) };
    if unsafe {
        overlaps(
            out as *const u8..(out as *const u8).offset(out_len as isize),
            input..input.offset(input_len as isize),
        )
    } {
        // Avoid allocations for small slices
        if input_len <= 256 {
            let mut tmp = [0u8; 256];
            unsafe {
                tmp[..input_len].copy_from_slice(slice::from_raw_parts(input, input_len));
                do_swap(output, &tmp[..input_len]);
            }
        } else {
            let mut tmp = vec![0u8; input_len];
            unsafe {
                tmp.as_mut_slice()
                    .copy_from_slice(slice::from_raw_parts(input, input_len));
                do_swap(output, &tmp);
            }
        }
    } else {
        unsafe {
            do_swap(output, slice::from_raw_parts(input, input_len));
        }
    }
}

fn do_swap(
    out: &mut [u8],
    input: &[u8],
) {
    debug_assert_eq!(out.len(), input.len());
    let len = input.len();
    for i in 0..len {
        out[i] = input[len - 1 - i];
    }
}
