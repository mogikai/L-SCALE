pub use glaze_mem as mem;
pub use glaze_mem::Mmu;
pub use lifter::DecodeError;

pub use crate::config::Config;
pub use crate::core::regs::{
    RegValue,
    Regs,
    ValueSource,
    VarSource,
};
pub use crate::core::trace::{
    HookHandler,
    HookTrampoline,
    InstHook,
    StoreRef,
    TraceStore,
};
pub use crate::core::{
    Arch,
    CpuCore,
    CpuSnapshot,
    Exception,
    RegHandler,
    ShadowStack,
    ShadowStackEntry,
};
pub use crate::cpu::exit::CpuExitStatus;
pub use crate::lifter::BlockGroup;

pub mod core;
pub mod debug_info;
pub mod exec;
pub mod lifter;

pub mod config;
pub mod cpu;
pub mod jit;
pub mod pcode;
pub mod sleigh;
pub mod utils;

#[derive(Copy, Clone)]
#[repr(u64)]
pub enum InternalError {
    CorruptedBlockMap,
    UnsupportedIsaMode,
}
