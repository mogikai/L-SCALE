use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use num_enum::{
    IntoPrimitive,
    TryFromPrimitive,
};

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// Represents all general purpose registers available on x64.
/// The size of the register is not encoded here. This enum only
/// serves as a selector for the register and not for its size.
pub enum GPRegister {
    AX  = 0,
    CX  = 1,
    DX  = 2,
    BX  = 3,
    SP  = 4,
    BP  = 5,
    SI  = 6,
    DI  = 7,
    R8  = 8,
    R9  = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

impl GPRegister {
    pub const fn into_host_reg(self) -> HostReg {
        HostReg::new(self as u8, RegClass::Int)
    }
}

impl From<GPRegister> for usize {
    fn from(value: GPRegister) -> Self {
        usize::from(u8::from(value))
    }
}
