#![allow(unused_variables)]
//! The x86 JIT implementation can make use of all registers, except RBP, since RBP stores the cpu context.
use crate::jit::api::{
    JITRuntimeContext,
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
use bitvec::array::BitArray;
use dynasmrt::x64::Assembler;
use dynasmrt::{
    dynasm,
    AssemblyOffset,
    DynasmApi,
    ExecutableBuffer,
};
use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::spec::jir::jcode::{
    JIRInstruction,
    JIROperation,
};
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_utilities::addresses::{
    EVAddr,
    EVAddress,
};
use num_enum::{
    FromPrimitive,
    IntoPrimitive,
    TryFromPrimitive,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem;
use std::rc::Rc;
use tracing::debug;

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
enum GPRegister {
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
struct UsedRegisters {
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

/// This struct is used to keep track of which slice is loaded in what register.
/// TODO: This most likely needs to be optimized for size.
#[derive(Debug, Default)]
struct RegisterTracker {
    gp: [Option<JIRSlice>; 16],
    fp: [Option<JIRSlice>; 8],
    mmx: [Option<JIRSlice>; 8],
    xmm: [Option<JIRSlice>; 16],
    ymm: [Option<JIRSlice>; 16],
}

#[derive(Debug)]
struct UnfinishedBasicBlock {
    start: EVAddr,
    assembler: Assembler,
    jmp_epilogue_offset: Option<AssemblyOffset>,
}

// TODO: In order to more easily create the basic blocks, it might make sense to
//       create a jumplist at the end of each basic block, where we can insert the
//       jumps to the following blocks

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
            used_registers: Default::default(),
        })
    }
}

#[derive(Debug)]
struct BasicBlock {
    start: EVAddr,
    code: ExecutableBuffer,
    jmp_epilogue_offset: Option<AssemblyOffset>,
    /// TODO: The used registers could be used during block chaining to eliminate flushes.
    ///       For example, if the parent block writes to a register, which is simply overwritten
    ///       in the child block, then this register never requires flushing.
    used_registers: UsedRegisters,
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

pub struct X86JITRuntimeContext {
    spaces: Vec<Space>,
    pc: usize,
}

impl JITRuntimeContext for X86JITRuntimeContext {
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

pub struct X86MogiJit {
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    unfinished_block: Option<UnfinishedBasicBlock>,
    /// The jit prelude is the entry point of the emulator. The jir prelude is called with the
    /// context as first argument, and the block to execute as second argument (see [PreludeSignature])
    jit_prelude: ExecutableBuffer,
    /// The jit epilogue is the exit point of the emulator. By default, blocks will chain to this
    /// special block, such that the execution returns to the main emulator loop.
    /// However, it is possible to directly link multiple jited blocks together, such that the execution
    /// remains in the jited code, skipping the main emulator loop. This is more efficient, but requires
    /// that the blocks have already been translated and are still valid.
    /// The epilogue will fully flush all registers into their corresponding varnodes.
    jit_epilogue: ExecutableBuffer,
    jit_cache: JitCache<BasicBlock>,
    /// The register tracker keeps track of the registers used in the currently constructed block.
    /// If all registers are occupied, one of the registers must be spilled into memory, to free
    /// up space for the current operation.
    register_tracker: RegisterTracker,
    mmu: Rc<RefCell<MogiMMU>>,
}

impl X86MogiJit {
    // TODO: Make MMU generic (put behind trait)
    pub fn new(
        processor_specification: Rc<RefCell<ProcessorSpecification>>,
        mmu: Rc<RefCell<MogiMMU>>,
    ) -> Self {
        Self {
            processor_specification,
            unfinished_block: Default::default(),
            jit_prelude: build_prelude().unwrap(),
            jit_epilogue: build_epilogue().unwrap(),
            jit_cache: JitCache::new(),
            register_tracker: Default::default(),
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
        let new_jmp = new_jmp.finalize().unwrap();
        let mut code = origin.code.make_mut()?;
        // Replace the jmp instruction with a new one pointing to the new target address
        code[offset..offset + new_jmp.len()].copy_from_slice(&new_jmp);
        origin.code = code.make_exec()?;
        // Done! We can now put the block back into the cache.
        self.jit_cache.populate(origin);
        Ok(())
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

    /// Emit code to load the requested slice into a [GPRegister] if necessary.
    /// If the slice is already loaded, the register is returned.
    /// Otherwise, code is emitted that loads the slice into a free register,
    /// which is then returned.
    fn load_slice_gp(
        &mut self,
        slice: JIRSlice,
    ) -> GPRegister {
        let mut free_gp = None;
        // Search for existing register, and otherwise take the first available GP register.
        for (i, entry) in self.register_tracker.gp.iter().enumerate() {
            if let Some(entry) = entry {
                if *entry == slice {
                    return GPRegister::try_from(i as u8).unwrap();
                }
            } else if free_gp.is_none() {
                free_gp = Some(GPRegister::try_from(i).unwrap());
            }
        }
        // TODO: Load gp. Handle spill case
        GPRegister::B
    }

    fn emit_copy(
        &mut self,
        target: JIRSlice,
        source: JIRValue,
    ) -> Result<(), JitError> {
        debug_assert_eq!(target.size(), source.size());

        let size = target.size();
        // Load the target slice into a GP register.
        let target_reg = self.load_slice_gp(target) as u8;
        match source {
            JIRValue::Slice(slice) => {
                let reg = self.load_slice_gp(slice) as u8;
                let block = self.block_mut()?;
                dynasm!(block.assembler
                    ; mov Rd(0), Rd(reg)
                );
                todo!()
            }
            JIRValue::Const(value) => {
                let value = value.value();
                let block = self.block_mut()?;
                match size {
                    1 => {
                        // 8 bit move
                        dynasm!(block.assembler
                            ; mov Rb(target_reg), value as _
                        );
                    }
                    2 => {
                        // 16 bit move
                        dynasm!(block.assembler
                            ; mov Rw(target_reg), value as _
                        );
                    }
                    3..=4 => {
                        // 32 bit move.
                        dynasm!(block.assembler
                            ; mov Rd(target_reg), value as _
                        );
                    }
                    5..=8 => {
                        // 64 bit move.
                        dynasm!(block.assembler
                            ; mov Rq(target_reg), value as _
                        );
                    }
                    _ => {
                        panic!("Invalid size")
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_add(
        &mut self,
        output: JIRSlice,
        a: JIRValue,
        b: JIRValue,
    ) -> Result<(), JitError> {
        let block = self.block_mut()?;
        dynasm!(block.assembler
            ; add Rd(0), 1
        );
        Ok(())
    }

    fn block_mut(&mut self) -> Result<&mut UnfinishedBasicBlock, JitError> {
        self.unfinished_block
            .as_mut()
            .ok_or(JitError::NoActiveBlock)
    }
}

pub struct X86Var {}

impl JitValue for X86Var {
    type Constant = ();
    type Variable = ();
}

impl MogiJit for X86MogiJit {
    type RuntimeContext = X86JITRuntimeContext;
    type Value = X86Var;

    fn new_runtime_context(&self) -> Self::RuntimeContext {
        X86JITRuntimeContext {
            spaces: vec![Space::new()],
            pc: 0,
        }
    }

    fn new_block(
        &mut self,
        evaddr: EVAddr,
    ) -> Result<(), JitError> {
        if self.unfinished_block.is_some() {
            todo!("Return error: 'Tried to create new block without finalizing the previous block'")
        }
        self.unfinished_block = Some(UnfinishedBasicBlock::new(evaddr));
        Ok(())
    }

    fn emit_jinst(
        &mut self,
        jinst: &JIRInstruction,
    ) -> Result<(), JitError> {
        let output = jinst.output.ok_or(JitError::MissingOutput);
        let input_a = jinst.input_a.ok_or(JitError::MissingInput);
        let input_b = jinst.input_b.ok_or(JitError::MissingInput);
        match jinst.operation {
            JIROperation::Unimpl => {}
            JIROperation::Copy => return self.emit_copy(output?, input_a?),
            JIROperation::Load(_) => {}
            JIROperation::Store(_) => {}
            JIROperation::IntAdd => return self.emit_add(output?, input_a?, input_b?),
            JIROperation::IntSubtract => {}
            JIROperation::IntMul => {}
            JIROperation::IntDiv => {}
            JIROperation::IntSDiv => {}
            JIROperation::IntRemainder => {}
            JIROperation::IntSRemainder => {}
            JIROperation::IntLeft => {}
            JIROperation::IntRight => {}
            JIROperation::IntSRight => {}
            JIROperation::IntOr => {}
            JIROperation::IntAnd => {}
            JIROperation::IntXor => {}
            JIROperation::IntSLess => {}
            JIROperation::IntSGreater => {}
            JIROperation::IntSLessEqual => {}
            JIROperation::IntSGreaterEqual => {}
            JIROperation::IntLess => {}
            JIROperation::IntGreater => {}
            JIROperation::IntLessEqual => {}
            JIROperation::IntGreaterEqual => {}
            JIROperation::IntEquals => {}
            JIROperation::IntNotEquals => {}
            JIROperation::BitNegate => {}
            JIROperation::IntNegate => {}
            JIROperation::FloatDiv => {}
            JIROperation::FloatMul => {}
            JIROperation::FloatAdd => {}
            JIROperation::FloatSubtract => {}
            JIROperation::FloatLess => {}
            JIROperation::FloatGreater => {}
            JIROperation::FloatLessEqual => {}
            JIROperation::FloatGreaterEqual => {}
            JIROperation::FloatEquals => {}
            JIROperation::FloatNotEquals => {}
            JIROperation::FloatNegate => {}
            JIROperation::BoolOr => {}
            JIROperation::BoolAnd => {}
            JIROperation::BoolXor => {}
            JIROperation::BoolNegate => {}
            JIROperation::CallDirect => {}
            JIROperation::CallIndirect => {}
            JIROperation::ReturnDirect => {}
            JIROperation::ReturnIndirect => {}
            JIROperation::GotoDirect => {}
            JIROperation::GotoIndirect => {}
            JIROperation::GotoDirectIf => {}
            JIROperation::GotoIndirectIf => {}
            JIROperation::JCodeGoto(_) => {}
            JIROperation::JCodeGotoIf(_) => {}
            JIROperation::PopCount => {}
            JIROperation::LZCount => {}
            JIROperation::ZeroExtend => {}
            JIROperation::SignExtend => {}
            JIROperation::Carry => {}
            JIROperation::SignCarry => {}
            JIROperation::SignBorrow => {}
            JIROperation::Nan => {}
            JIROperation::Abs => {}
            JIROperation::Sqrt => {}
            JIROperation::IntToFloat => {}
            JIROperation::FloatToFloat => {}
            JIROperation::Trunc => {}
            JIROperation::Ceil => {}
            JIROperation::Floor => {}
            JIROperation::Round => {}
            JIROperation::CustomOp { .. } => {}
        }
        todo!()
    }

    // fn emit_load(
    //     &mut self,
    //     ty: (),
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    // ) -> Result<Self::Value, JitError> {
    //     todo!()
    // }
    //
    // fn emit_store(
    //     &mut self,
    //     address: Self::Value,
    //     offset: <Self::Value as JitValue>::Constant,
    //     value: Self::Value,
    // ) -> Result<(), JitError> {
    //     todo!()
    // }

    // fn emit_ret(&mut self) -> Result<(), JitError> {
    //     let block = self.current_block()?;
    //     dynasm!(block.assembler
    //         ; ret
    //     );
    //     Ok(())
    // }

    // fn emit_pop(
    //     &mut self,
    //     var: Self::Value,
    // ) -> Result<(), JitError> {
    //     // TODO: Every basic block has "outputs" and "inputs"
    //     //       "outputs" are simply variables that have been written to during the blocks execution.
    //     //       "inputs" are variables that are read from, before they are written to during the blocks execution.
    //     // TODO: self.allocate_registers()?
    //     //       Need to check if var is already a register (e.g. context is fixed as a register)
    //     //       If it is not a register, it needs to be loaded from the cpu state
    //     todo!()
    //     // TODO: self.sync_registers()?
    //     //       we might need to write registers back to the cpu state, depending on if they
    //     //       are overwritten before the block ends or not.
    //     //       Effectively, all "output" registers need to be written back.
    //     //       Whether or not a variable is "dead" or an "output", is set by the MIR optimizer.
    //     //       The state is stored in the variable struct.
    //     //       In order to do this, we need to write a store instruction to host assembly.
    // }

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

    fn finalize_block(&mut self) -> Result<(), JitError> {
        let Some(mut block) = self.unfinished_block.take() else {
            todo!("Return error: 'Cannot finalize because no block was started'")
        };
        // Write the final jump instruction to the jit epilogue.
        // This can later be overridden to jump directly to the next block.
        block.jmp_epilogue_offset = Some(block.assembler.offset());
        dynasm!(block.assembler
            // ; mov rax, QWORD epilogue as _
            ; jmp DWORD 0
        );
        let block = block.finalize()?;
        let addr = self.jit_cache.populate(block);
        self.chain_blocks_direct(addr, self.jit_epilogue.as_ptr())?;
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
        // For debugging we break on jumping into the prelude.
        // TODO: Make this configurable.
        // ; int1
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
