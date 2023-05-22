#[macro_export]
macro_rules! push_stack {
    ($cpu:ident, $data:expr, $reg:expr) => {{
        $cpu.reg_read($reg).map_err(|e| GenericError::from(e)).and_then(|sp| {
            MemoryAccessor::<Endianness, _>::new($cpu)
                .write_direct_down(sp.into(), $data)
                .map_err(|e| GenericError::from(e))
                .and_then(|len| {
                    let diff = sp.wrapping_sub(len as u64);
                    $cpu.reg_write($reg, diff).map_err(|e| GenericError::from(e)).and(Ok(diff))
                })
        })
    }};
}

#[macro_export]
macro_rules! read_osstring {
    ($mmu:expr, $addr:expr) => {{
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use $crate::emulator::abstractions::mmu::MemoryError;
        //todo better define path_max
        $mmu.mem_read_bytes($addr, 4096).and_then(|bytes| {
            // TODO: This should be a more specific memory error. String not null terminated or smth
            bytes
                .split(|c| *c == 0)
                .next()
                .ok_or(MemoryError::Unknown)
                .and_then(|string| Ok(OsString::from_vec(string.to_vec())))
        })
    }};
}
