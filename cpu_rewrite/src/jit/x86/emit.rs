//! This module contains all the functions that are used to emit code for [JIRInstruction]s

mod arithmetic;
mod bitwise;
mod host_call;
mod load;
mod logic;
mod mmu;

/// Emit a 64 bit mov instruction, but only if the registers differ.
macro_rules! mov64 {
    ($asm:expr; $dest_reg:expr, $src_reg:expr) => {
        use dynasmrt::DynasmApi;
        use dynasmrt::dynasm;
        if $dest_reg != $src_reg {
            assemble!($asm;
                ; mov Rq($dest_reg), Rq($src_reg)
            );
        }
    };
}

use mov64;

pub use host_call::CustomOpCallback;
