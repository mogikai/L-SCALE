mod add;
mod divide;
mod multiply;
mod negate;
mod shift;
mod subtract;
#[cfg(test)]
mod test;

use crate::jit::api::JITResult;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

enum AddSub {
    Add,
    Sub,
}
