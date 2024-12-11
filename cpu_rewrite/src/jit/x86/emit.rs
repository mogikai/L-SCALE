//! This module contains all the functions that are used to emit code for [JIRInstruction]s

use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::x86::{
    GPRegister,
    X86MogiJit,
    assemble,
};
use dynasmrt::dynasm;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use std::cmp::min;

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
