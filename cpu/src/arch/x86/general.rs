//! General helper implementations

pub extern "sysv64" fn swap_bytes(a: u64) -> u64 {
    // TODO: This only works for 64 bit swaps. We need a way to implement this for other slice sizes as well
    //       One option would be to pass the size as an extra parameter. However, this would require loading
    //       the size for each slice into a register. If possible we should avoid this.
    //       Another could be to have a separate function for each slice size. This is feasible since we are
    //       dealing with sizes one through 8 (bytes).
    //       Another possible optimization could be to compile the functions without a function prologue and epilogue
    //       with a custom calling convention. That way we could avoid clobbering as many registers as we currently do.
    a.swap_bytes()
}
