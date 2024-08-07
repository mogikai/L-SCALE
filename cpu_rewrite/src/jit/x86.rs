//! The x86 JIT implementation can make use of all registers, except RBP, since RBP stores the cpu context.
use crate::jit::api::{
    JitContext,
    JitError,
    JitValue,
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
use mogi_mem::mmu::MogiMMU;
use mogi_utilities::addresses::{
    EVAddr,
    EVAddress,
};
use slotmap::{
    new_key_type,
    SlotMap,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem;
use std::rc::Rc;

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

#[repr(C)]
struct Space {
    /// The storage vec contains all registers and temporaries in a serialized form.
    /// Each variable has an index and size.
    storage: [u8; 0x1000],
}

impl Space {
    fn new() -> Self {
        Self { storage: [0u8; 0x1000] }
    }
}

pub struct X86JitContext {
    spaces: Vec<Space>,
    pc: usize,
}

impl JitContext for X86JitContext {
    fn pc(&self) -> EVAddr {
        EVAddr::null()
    }

    fn register(
        &self,
        register: (),
    ) -> () {
        todo!()
    }

    fn register_mut(
        &mut self,
        register: (),
    ) -> () {
        todo!()
    }
}

new_key_type! { pub struct BlockId; }
pub struct X86MogiJit {
    current_block: Option<BlockId>,
    unfinished_blocks: SlotMap<BlockId, UnfinishedBasicBlock>,
    jit_prelude: ExecutableBuffer,
    jit_epilogue: ExecutableBuffer,
    jit_cache: JitCache<BasicBlock>,
    mmu: Rc<RefCell<MogiMMU>>,
}

impl X86MogiJit {
    // TODO: Make MMU generic (put behind trait)
    pub fn new(mmu: Rc<RefCell<MogiMMU>>) -> Self {
        Self {
            current_block: None,
            unfinished_blocks: Default::default(),
            jit_prelude: build_prelude().unwrap(),
            jit_epilogue: build_epilogue().unwrap(),
            jit_cache: JitCache::new(),
            mmu,
        }
    }
}

impl X86MogiJit {
    /// Directly chain a target address to the block at the provided EVAddr.
    fn chain_blocks_direct(
        &mut self,
        origin_block_address: EVAddr,
        target_address: *const u8,
    ) -> Result<(), JitError> {
        // We need to take the block out of the cache, in order to modify it.
        let mut origin = self.jit_cache.take(origin_block_address)?;
        let Some(epilogue_offset) = origin.jmp_epilogue_offset else {
            Err(JitError::NoEpilogue)?
        };
        // The origin address is after the jump instruction, hence we add 5 here.
        // See the intel manual for relative jmp size with 32 bit argument.
        let orign_address = unsafe { origin.code.ptr(epilogue_offset).add(5) };
        let jmp_offset = i32::try_from(unsafe { target_address.offset_from(orign_address) }).unwrap();
        // We know this must be safe, since the offset points into the memory starting at code.as_ptr
        let offset = unsafe {
            origin
                .code
                .ptr(epilogue_offset)
                .offset_from(origin.code.as_ptr()) as usize
        };
        let mut new_jmp = Assembler::new()?;
        dynasm!(new_jmp
        ; jmp jmp_offset as _
        );
        let new_mov = new_jmp.finalize().unwrap();
        let mut code = origin.code.make_mut()?;
        // Replace the jmp instruction with a new one pointing to the new target address
        code[offset..new_mov.len()].copy_from_slice(&new_mov);
        origin.code = code.make_exec()?;
        // Done! We can now put the block back into the cache.
        self.jit_cache.populate(origin);
        Ok(())
    }

    fn current_block(&mut self) -> Result<&mut UnfinishedBasicBlock, JitError> {
        Ok(self
            .unfinished_blocks
            .get_mut(self.current_block.ok_or(JitError::NoActiveBlock)?)
            .expect("The current block must be in the unfinished blocks arena"))
    }

    // fn emit_host_load(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }
    //
    // fn emit_host_store(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }

    // fn emit_load_fast_path(
    //     &mut self,
    //     target_register: u8,
    // ) -> Result<(), JitError> {
    //     let block = self.current_block()?;
    //     dynasm!(block.assembler
    //         ; mov Rq(target_register), 0
    //     );
    //     Ok(())
    // }
}

pub struct X86Var {}

impl JitValue for X86Var {
    type Constant = ();
    type Variable = ();
}

impl MogiJit for X86MogiJit {
    type BlockId = BlockId;
    type Context = X86JitContext;
    type Value = X86Var;

    fn new_context(&self) -> Self::Context {
        X86JitContext {
            spaces: vec![Space::new()],
            pc: 0,
        }
    }

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

    fn emit_load(
        &mut self,
        ty: (),
        address: Self::Value,
        offset: <Self::Value as JitValue>::Constant,
    ) -> Result<Self::Value, JitError> {
        todo!()
    }

    fn emit_store(
        &mut self,
        address: Self::Value,
        offset: <Self::Value as JitValue>::Constant,
        value: Self::Value,
    ) -> Result<(), JitError> {
        todo!()
    }

    fn emit_ret(&mut self) -> Result<(), JitError> {
        let block = self.current_block()?;
        dynasm!(block.assembler
            ; ret
        );
        Ok(())
    }

    fn emit_push(
        &mut self,
        var: Self::Value,
    ) -> Result<(), JitError> {
        todo!()
    }

    fn emit_pop(
        &mut self,
        var: Self::Value,
    ) -> Result<(), JitError> {
        // TODO: Every basic block has "outputs" and "inputs"
        //       "outputs" are simply variables that have been written to during the blocks execution.
        //       "inputs" are variables that are read from, before they are written to during the blocks execution.
        // TODO: self.allocate_registers()?
        //       Need to check if var is already a register (e.g. context is fixed as a register)
        //       If it is not a register, it needs to be loaded from the cpu state
        todo!()
        // TODO: self.sync_registers()?
        //       we might need to write registers back to the cpu state, depending on if they
        //       are overwritten before the block ends or not.
        //       Effectively, all "output" registers need to be written back.
        //       Whether or not a variable is "dead" or an "output", is set by the MIR optimizer.
        //       The state is stored in the variable struct.
        //       In order to do this, we need to write a store instruction to host assembly.
    }

    fn emit_jmp_epilogue(&mut self) -> Result<(), JitError> {
        let epilogue: *const c_void = self.jit_epilogue.as_ptr() as _;
        let block = self.current_block()?;
        block.jmp_epilogue_offset = Some(block.assembler.offset());
        dynasm!(block.assembler
            // ; mov rax, QWORD epilogue as _
            ; jmp DWORD 0
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
        let target_address = unsafe {
            self.jit_cache
                .get_block_code(target_block_address)?
                .raw_ptr()
        };
        self.chain_blocks_direct(origin_block_address, target_address)
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
        let block_address = block.ev_address();
        self.jit_cache.populate(block);
        self.chain_blocks_direct(block_address, self.jit_epilogue.as_ptr())?;
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
