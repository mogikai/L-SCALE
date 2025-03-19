use crate::jit::host::register::{
    HostReg,
    HostRegClass,
};
use crate::jit::x86::registers::{
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
        HostReg::new(self as u8, HostRegClass::Int)
    }
}

impl InitialAllocation for GPRegister {
    fn initial_allocation(&self) -> RegAlloc {
        let ty = match self {
            // we reserve RAX as temporary since there are some instructions that can only use RAX
            GPRegister::AX => RegAllocType::Reserved,
            // RBP is reserved for the cpu_context
            GPRegister::BP => RegAllocType::Reserved,
            // We don't want to mess with the stack pointer.
            // TODO: It may make sense to keep the emulated stack pointer loaded in some register.
            //       This might even be the host SP register, but we will have to save the value somewhere.
            //       Maybe it even makes sense to provide an api where one can specify target registers
            //       that should be loaded into fixed host registers upon entering the jit. (SP?, FLAGS?)
            GPRegister::SP => RegAllocType::Reserved,
            _ => RegAllocType::Free,
        };
        RegAlloc { ty, version: 0 }
    }
}

impl From<GPRegister> for usize {
    fn from(value: GPRegister) -> Self {
        usize::from(u8::from(value))
    }
}
