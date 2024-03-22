use std::any::Any;

use hashbrown::{
    HashMap,
    HashSet,
};

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

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct BlockKey {
    pub vaddr: u64,
    pub isa_mode: u64,
}

/// Keeps track of all the code in the program that the emulator has discovered.
#[derive(Default)]
pub struct BlockTable {
    pub map: HashMap<BlockKey, BlockGroup>,
    pub blocks: Vec<lifter::Block>,
    pub disasm: HashMap<u64, String>,
    pub breakpoints: HashSet<u64>,
    pub modified: HashSet<usize>,
}

impl BlockTable {
    pub fn flush_code(&mut self) {
        self.map.clear();
        self.blocks.clear();
        self.disasm.clear();
        self.modified.clear();
    }

    pub fn get_info(
        &self,
        key: BlockKey,
    ) -> Option<BlockInfoRef> {
        let group = *self.map.get(&key)?;
        Some(BlockInfoRef { group, code: self })
    }

    pub fn address_of(
        &self,
        id: u64,
        offset: u64,
    ) -> u64 {
        let block = &self.blocks[id as usize];
        block
            .pcode
            .instructions
            .iter()
            .take(offset as usize)
            .filter(|inst| matches!(inst.op, pcode::Op::InstructionMarker))
            .map(|x| x.inputs.first().as_u64())
            .last()
            .unwrap_or(0)
    }
}

pub struct BlockInfoRef<'a> {
    group: BlockGroup,
    code: &'a BlockTable,
}

impl<'a> BlockInfoRef<'a> {
    /// Return an iterator over all (instruction start, instruction len) pairs in the group.
    pub fn instructions(&self) -> impl Iterator<Item = (u64, u64)> + 'a {
        self.code.blocks[self.group.range()]
            .iter()
            .flat_map(|block| block.instructions())
    }

    /// Return the entry block
    pub fn entry_block(&self) -> &'a lifter::Block {
        &self.code.blocks[self.group.blocks.0]
    }

    /// Return an iterator over all blocks in the group
    pub fn blocks(&self) -> impl Iterator<Item = (usize, &'a lifter::Block)> + 'a {
        self.group.range().map(|i| (i, &self.code.blocks[i]))
    }
}

#[derive(Copy, Clone)]
#[repr(u64)]
pub enum InternalError {
    CorruptedBlockMap,
    UnsupportedIsaMode,
}

pub fn read_value_zxt(
    cpu: &mut CpuCore,
    value: pcode::Value,
) -> u64 {
    cpu.read_dynamic(value).zxt()
}

pub fn read_value_sxt(
    cpu: &mut CpuCore,
    value: pcode::Value,
) -> i64 {
    cpu.read_dynamic(value).sxt()
}
