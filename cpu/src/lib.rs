pub mod core;
pub mod debug_info;
pub mod exec;
pub mod lifter;

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
