//! The x86 JIT implementation can make use of all registers, except RBP, since RBP stores the cpu context.
use crate::jit::api::{
    JitError,
    MogiJit,
    PreludeSignature,
};
use crate::jit::cache::{
    AsJitCode,
    JitCache,
    JitCode,
};
use dynasmrt::x64::Assembler;
use dynasmrt::{
    dynasm,
    AssemblyOffset,
    DynasmApi,
    ExecutableBuffer,
};
use mogi_utilities::addresses::{
    EVAddr,
    EVAddress,
};
use slotmap::{
    new_key_type,
    SlotMap,
};
use std::ffi::c_void;
use std::mem;

#[derive(Debug)]
struct UnfinishedBasicBlock {
    start: EVAddr,
    assembler: Assembler,
    jmp_epilogue_offset: Option<AssemblyOffset>,
}

impl UnfinishedBasicBlock {
    pub fn new(start: EVAddr) -> Self {
        Self {
            start,
            assembler: Assembler::new().unwrap(),
            jmp_epilogue_offset: None,
        }
    }

    pub fn finalize(mut self) -> Result<BasicBlock, JitError> {
        self.assembler
            .commit()
            .map_err(|_| JitError::AssemblerError)?;
        Ok(BasicBlock {
            start: self.start,
            code: self.assembler.finalize().unwrap(),
            jmp_epilogue_offset: self.jmp_epilogue_offset,
        })
    }
}

#[derive(Debug)]
struct BasicBlock {
    start: EVAddr,
    code: ExecutableBuffer,
    jmp_epilogue_offset: Option<AssemblyOffset>,
}

impl AsJitCode for BasicBlock {
    fn as_jit_code(&self) -> JitCode {
        JitCode::from(self.code.as_ptr() as *const c_void)
    }
}

impl EVAddress for BasicBlock {
    fn ev_address(&self) -> EVAddr {
        self.start
    }
}

new_key_type! { pub struct BlockId; }
pub struct X86MogiJit {
    current_block: Option<BlockId>,
    unfinished_blocks: SlotMap<BlockId, UnfinishedBasicBlock>,
    jit_prelude: ExecutableBuffer,
    jit_epilogue: ExecutableBuffer,
    jit_cache: JitCache<BasicBlock>,
}

impl Default for X86MogiJit {
    fn default() -> Self {
        Self::new()
    }
}

impl X86MogiJit {
    pub fn new() -> Self {
        Self {
            current_block: None,
            unfinished_blocks: Default::default(),
            jit_prelude: build_prelude().unwrap(),
            jit_epilogue: build_epilogue().unwrap(),
            jit_cache: JitCache::new(),
        }
    }
}

impl X86MogiJit {
    fn current_block(&mut self) -> Result<&mut UnfinishedBasicBlock, JitError> {
        Ok(self
            .unfinished_blocks
            .get_mut(self.current_block.ok_or(JitError::NoActiveBlock)?)
            .expect("The current block must be in the unfinished blocks arena"))
    }
}

impl MogiJit for X86MogiJit {
    type BlockId = BlockId;
    fn new_block(
        &mut self,
        evaddr: EVAddr,
    ) -> Result<Self::BlockId, JitError> {
        Ok(self
            .unfinished_blocks
            .insert(UnfinishedBasicBlock::new(evaddr)))
    }

    fn activate_block(
        &mut self,
        block_id: Self::BlockId,
    ) -> Result<(), JitError> {
        let _ = self.current_block.insert(block_id);
        Ok(())
    }

    fn emit_ret(&mut self) -> Result<(), JitError> {
        let block = self.current_block()?;
        dynasm!(block.assembler
            ; ret
        );
        Ok(())
    }

    fn emit_jmp_epilogue(&mut self) -> Result<(), JitError> {
        let epilogue: *const c_void = self.jit_epilogue.as_ptr() as _;
        let block = self.current_block()?;
        block.jmp_epilogue_offset = Some(block.assembler.offset());
        dynasm!(block.assembler
            ; mov rax, QWORD epilogue as _
            ; jmp rax
        );
        Ok(())
    }

    fn chain_blocks(
        &mut self,
        origin_block_address: EVAddr,
        target_block_address: EVAddr,
    ) -> Result<(), JitError> {
        // This is safe, since we only relocate the target block if we need to update the code.
        // This requires unchaining the block, which will remove this address and replace it with
        // the epilogue.
        let target = unsafe {
            self.jit_cache
                .get_block_code(target_block_address)?
                .raw_ptr()
        };
        let mut origin = self.jit_cache.take(origin_block_address)?;
        let Some(epilogue_offset) = origin.jmp_epilogue_offset else {
            Err(JitError::NoEpilogue)?
        };
        // We know this must be safe, since the offset points into the memory starting at code.as_ptr
        let offset = unsafe {
            origin
                .code
                .ptr(epilogue_offset)
                .offset_from(origin.code.as_ptr()) as usize
        };
        let mut new_mov = Assembler::new()?;
        dynasm!(new_mov
        ; mov rax, QWORD target as _
        );
        let new_mov = new_mov.finalize().unwrap();
        let mut code = origin.code.make_mut()?;
        // Replace the mov instruction that moves the instruction address into RAX for the subsequent jmp
        // effectively changing the jmp target from epilogue to the target block.
        code[offset..new_mov.len()].copy_from_slice(&new_mov);
        origin.code = code.make_exec()?;
        self.jit_cache.populate(origin);
        Ok(())
    }

    fn finalize_block(
        &mut self,
        block_id: Self::BlockId,
    ) -> Result<(), JitError> {
        let block = self
            .unfinished_blocks
            .remove(self.current_block.take().ok_or(JitError::NoActiveBlock)?)
            .expect("The current block must be in the unfinished blocks arena")
            .finalize()
            .unwrap();
        self.jit_cache.populate(block);
        Ok(())
    }

    fn fetch_block(
        &self,
        evaddr: EVAddr,
    ) -> Result<JitCode, JitError> {
        self.jit_cache
            .get_block_code(evaddr)
            .map_err(JitError::from)
    }

    fn prelude(&self) -> Result<PreludeSignature, JitError> {
        let jit_code: PreludeSignature = unsafe { mem::transmute(self.jit_prelude.as_ptr()) };
        Ok(jit_code)
    }
}

/// This function builds the jit prelude. It is a function that is called in order to enter the jited code.
/// The prelude is called like a C function, and thus only callee saved registers need to be preserved inside
/// the call. The Signature of the jit prelude function can be found here: [PreludeSignature]
fn build_prelude() -> Result<ExecutableBuffer, JitError> {
    let mut asm = Assembler::new()?;
    dynasm!(asm
        // See the System V ABI specification: https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf
        // Push callee saved registers onto stack
        ; push rbp
        ; push rbx
        ; push r12
        ; push r13
        ; push r14
        ; push r15
        // Move context to rbp. Function arguments are passed in RDI, RSI, RDX, RCX, R8, R9
        ; mov rbp, rdi
        // Jump to provided block
        ; jmp rsi
    );
    // TODO: It might be necessary to put additional info onto the stack or reserve some space for tmp values?
    Ok(asm.finalize().unwrap())
}

/// Builds the epilogue of the jit. This block is jumped to, when the emulated code wants to exit into the emulator.
/// The code pops all the saved registers and then returns into the function that called the prelude.
fn build_epilogue() -> Result<ExecutableBuffer, JitError> {
    let mut asm = Assembler::new()?;
    dynasm!(asm
        ; pop r15
        ; pop r14
        ; pop r13
        ; pop r12
        ; pop rbx
        ; pop rbp
        ; ret
    );
    Ok(asm.finalize().unwrap())
}

impl From<std::io::Error> for JitError {
    fn from(value: std::io::Error) -> Self {
        // TODO: Pass through error message
        JitError::IOError
    }
}
