macro_rules! push_stack {
    ($accessor:expr, $cpu:ident, $data:expr, $reg:expr) => {{
        use crate::emulator::system::memory::rw_interface::DirectMemoryWrite;
        $cpu.reg_read($reg)
            .map_err(|e| GenericError::from(e))
            .and_then(|sp| {
                trace!("Writing to stack pointer {sp:#x?}");
                $accessor
                    .write_direct_down(sp.into(), $data)
                    .map_err(|e| GenericError::from(e))
                    .and_then(|len| {
                        let new_sp = sp.wrapping_sub(len as u64);
                        $cpu.reg_write($reg, new_sp)
                            .map_err(|e| GenericError::from(e))
                            .and(Ok(new_sp))
                    })
            })
    }};
}

macro_rules! read_osstring {
    ($memory:expr, $addr:expr) => {{
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use $crate::emulator::abstractions::mmu::MemoryError;
        //todo better define path_max
        $memory.read_bytes($addr, 4096).and_then(|bytes| {
            // TODO: This should be a more specific memory error. String not null terminated or smth
            bytes
                .split(|c| *c == 0)
                .next()
                .ok_or(MemoryError::Unknown)
                .and_then(|string| Ok(OsString::from_vec(string.to_vec())))
        })
    }};
}

pub(super) use {
    push_stack,
    read_osstring,
};
