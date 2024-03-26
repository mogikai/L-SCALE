pub mod core;
pub mod debug_info;
pub mod exec;
pub mod lifter;

pub mod cpu;
pub mod interface;
pub mod jit;
pub mod pcode;
pub mod sleigh;
pub mod utils;

pub use target_lexicon::Triple;

#[derive(Copy, Clone)]
#[repr(u64)]
pub enum InternalError {
    CorruptedBlockMap,
    UnsupportedIsaMode,
}
