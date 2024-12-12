use bitvec::array::BitArray;
use mogi_sleigh::spec::jir::values::JIRSlice;
use num_enum::{
    IntoPrimitive,
    TryFromPrimitive,
};
use std::ops::{
    Index,
    IndexMut,
};
use std::{
    array,
    mem,
};

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum GPRegister {
    A   = 0,
    C   = 1,
    D   = 2,
    B   = 3,
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

impl From<GPRegister> for usize {
    fn from(value: GPRegister) -> Self {
        usize::from(u8::from(value))
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum FPRegister {
    ST0 = 0,
    ST1 = 1,
    ST2 = 2,
    ST3 = 3,
    ST4 = 4,
    ST5 = 5,
    ST6 = 6,
    ST7 = 7,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum MMXRegister {
    MMX0 = 0,
    MMX1 = 1,
    MMX2 = 2,
    MMX3 = 3,
    MMX4 = 4,
    MMX5 = 5,
    MMX6 = 6,
    MMX7 = 7,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum XMMRegister {
    XMM0  = 0,
    XMM1  = 1,
    XMM2  = 2,
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

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum YMMRegister {
    YMM0  = 0,
    YMM1  = 1,
    YMM2  = 2,
    YMM4  = 4,
    YMM5  = 5,
    YMM6  = 6,
    YMM7  = 7,
    YMM8  = 8,
    YMM9  = 9,
    YMM10 = 10,
    YMM11 = 11,
    YMM12 = 12,
    YMM13 = 13,
    YMM14 = 14,
    YMM15 = 15,
}

#[derive(Debug, Default)]
pub struct UsedRegisters {
    /// 16 GP registers
    gp: BitArray<[u16; 1]>,
    /// 8 FP registers
    fp: BitArray<[u8; 1]>,
    /// 8 MMX registers
    mmx: BitArray<[u8; 1]>,
    /// 16 XMM registers
    xmm: BitArray<[u16; 1]>,
    /// 16 YMM registers
    ymm: BitArray<[u16; 1]>,
}

enum RegisterInfo {
    Reserved,
    Free,
    Occupied(JIRSlice),
}

#[derive(Debug)]
pub struct GPRegisterAllocations([RegisterAllocation; 16]);

impl Default for GPRegisterAllocations {
    fn default() -> Self {
        Self(array::from_fn(|i| {
            let reg = GPRegister::try_from(i as u8).expect("We expect the GP registers to be 0..16");
            match reg {
                // we reserve RAX as temporary since there are some instructions that can only use RAX
                GPRegister::A => RegisterAllocation::Reserved,
                // RBP is reserved for the cpu_context
                GPRegister::BP => RegisterAllocation::Reserved,
                // We don't want to mess with the stack pointer.
                // TODO: It may make sense to keep the emulated stack pointer loaded in some register.
                //       This might even be the host SP register, but we will have to save the value somewhere.
                //       Maybe it even makes sense to provide an api where one can specify target registers
                //       that should be loaded into fixed host registers upon entering the jit. (SP?, FLAGS?)
                GPRegister::SP => RegisterAllocation::Reserved,
                _ => RegisterAllocation::Free,
            }
        }))
    }
}

impl GPRegisterAllocations {
    pub fn iter(&self) -> impl Iterator<Item = (GPRegister, &RegisterAllocation)> {
        self.0.iter().enumerate().map(|(i, entry)| {
            (GPRegister::try_from(i as u8).expect("We iterate over the GP ids, hence this must not error"), entry)
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (GPRegister, &mut RegisterAllocation)> {
        self.0.iter_mut().enumerate().map(|(i, entry)| {
            (GPRegister::try_from(i as u8).expect("We iterate over the GP ids, hence this must not error"), entry)
        })
    }
}

impl Index<GPRegister> for GPRegisterAllocations {
    type Output = RegisterAllocation;

    fn index(
        &self,
        index: GPRegister,
    ) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl IndexMut<GPRegister> for GPRegisterAllocations {
    fn index_mut(
        &mut self,
        index: GPRegister,
    ) -> &mut Self::Output {
        &mut self.0[index as usize]
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RegisterAllocation {
    Free,
    Reserved,
    /// The slot is occupied by a temporary. These can simply be discarded if no longer needed
    OccupiedTemp,
    /// The slot is occupied by a slice. The slice needs to be flushed to memory if the slot is cleared.
    Occupied(JIRSlice),
}

/// This struct is used to keep track of which slice is loaded in what register.
/// TODO: This most likely needs to be optimized for size.
#[derive(Debug, Default)]
pub struct RegisterTracker {
    pub gp: GPRegisterAllocations,
    fp: [Option<JIRSlice>; 8],
    mmx: [Option<JIRSlice>; 8],
    xmm: [Option<JIRSlice>; 16],
    ymm: [Option<JIRSlice>; 16],
}

impl RegisterTracker {
    /// Resets all GP register slots to empty and returns the array of slices that were previously associated with
    /// the registers. This can be used to emit the flush instructions.
    pub fn reset_gp(&mut self) -> GPRegisterAllocations {
        mem::take(&mut self.gp)
    }
}

pub enum RegisterLoadInfo<T> {
    Unloaded(T),
    Loaded(T),
    NeedRelocation { from: T, to: T },
}
