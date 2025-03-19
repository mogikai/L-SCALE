use crate::jit::host::register::HostRegClass;
use crate::jit::x86::registers::{
    HostReg,
    InitialAllocation,
    RegAlloc,
    RegAllocType,
};
use num_enum::{
    IntoPrimitive,
    TryFromPrimitive,
};

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// This enum represents the available XMM, YMM, and ZMM Registers. (YMM and ZMM are extensions of XMM)
/// These registers are primarily used for floating point arithmetic and simd operations.
pub enum XMMRegister {
    XMM0  = 0,
    XMM1  = 1,
    XMM2  = 2,
    XMM3  = 3,
    XMM4  = 4,
    XMM5  = 5,
    XMM6  = 6,
    XMM7  = 7,
    XMM8  = 8,
    XMM9  = 9,
    XMM10 = 10,
    XMM11 = 11,
    XMM12 = 12,
    XMM13 = 13,
    XMM14 = 14,
    XMM15 = 15,
}

impl XMMRegister {
    pub const fn into_host_reg(self) -> HostReg {
        HostReg::new(self as u8, HostRegClass::Vector)
    }
}

impl From<XMMRegister> for usize {
    fn from(value: XMMRegister) -> Self {
        usize::from(u8::from(value))
    }
}

impl InitialAllocation for XMMRegister {
    fn initial_allocation(&self) -> RegAlloc {
        let ty = match self {
            _ => RegAllocType::Free,
        };
        RegAlloc { ty, version: 0 }
    }
}
