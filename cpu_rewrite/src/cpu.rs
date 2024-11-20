//! The main cpu struct. This is the public interface that will be visible to the end user.
//! The `CPUContext` contains all the runtime information needed during emulation.
//! This includes for example the registers and a pointer to the currently active `TLB`.

use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Debug;
use std::rc::Rc;

use thiserror::Error;

use mogi_mem::mmu::MogiMMU;
use mogi_sleigh::jinst;
use mogi_sleigh::spec::jir::values::JIRSlice;
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_utilities::addresses::EVAddr;

use crate::jit::api::{
    JITRuntimeContext,
    JitError,
    MogiJit,
};
use crate::jit::cache::JitCode;

#[derive(Error, Debug)]
pub enum StopReason {
    #[error("The cpu encountered the halt instruction")]
    Halt,
    #[error("The user aborted execution")]
    UserAbort,
    #[error("An interrupt was encountered")]
    Interrupt,
    #[error("The just in time compiler encountered an error")]
    JitError(#[from] JitError),
}

#[allow(dead_code)]
#[derive(Debug)]
enum MustInterpret {
    Yes,
    No,
}

#[repr(C)]
pub struct CPU<JitT>
where
    JitT: MogiJit,
{
    /// The jit context. Each jit implementation may have its own context structure, hence this is dependent on the
    /// used jit. TODO: It might make sense to have the context as a pointer, such that we can have multiple contexts
    ///                 that can be hot-swapped on switching to a different task? This would require putting a
    ///                 reference to the cpu inside the context, such that it can be accessed in callbacks.
    ///                 Alternatively, the cpu can be omitted and each callback has to have its own context pointer.
    runtime_context: Box<JitT::RuntimeContext>,
    /// The jit is behind a refcell, so we can guarantee that the cpu can be mutably accessed by the jited code,
    /// but the jit itself may not be modified during the codes runtime.
    jit: RefCell<JitT>,
    mmu: Rc<RefCell<MogiMMU>>,
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    mir_cache: (),
}

impl<JitT> CPU<JitT>
where
    JitT: MogiJit,
{
    pub fn new(
        jit: JitT,
        processor_specification: Rc<RefCell<ProcessorSpecification>>,
        mmu: Rc<RefCell<MogiMMU>>,
    ) -> Self {
        Self {
            runtime_context: Box::new(jit.new_runtime_context()),
            jit: RefCell::new(jit),
            mmu,
            processor_specification,
            mir_cache: (),
        }
    }

    /// Runs the CPU until it encounters a stop condition.
    /// This can be an instruction count limit or an interrupt.
    pub fn run(&mut self) -> Result<(), StopReason> {
        loop {
            self.handle_interrupt()?;
            // This populates the jit block cache for the next instruction(s)
            self.translate_blocks()?;
            match self.must_interpret() {
                MustInterpret::No => {
                    let context = &mut *self.runtime_context as *mut _ as *mut c_void;
                    let jit = self.jit.borrow();
                    let block = jit.fetch_block(self.runtime_context.pc())?;
                    jit.prelude()?(context, block);
                }
                MustInterpret::Yes => {
                    todo!()
                }
            }
        }
    }
}

impl<JitT> CPU<JitT>
where
    JitT: MogiJit,
{
    fn handle_interrupt(&mut self) -> Result<(), StopReason> {
        Ok(())
    }

    /// Checks if the current code needs to be interpreted, and not jited.
    fn must_interpret(&self) -> MustInterpret {
        // Check for breakpoints. If breakpoint is inside a block, that block needs to be interpreted until that step.
        MustInterpret::No
    }

    /// Translate the next block or blocks.
    fn translate_blocks(&mut self) -> Result<(), StopReason> {
        // // TODO: Return translate error instead of stop reason?
        // // Load target instructions into mir cache
        // let new_blocks = self.mir_cache.populate(self.context.pc());
        // // Translate to host blocks
        // let blocks = new_blocks.translate();
        // self.jit_cache.populate(blocks);
        {
            // TODO: Remove demo block
            let mut jit = self.jit.borrow_mut();
            if jit.fetch_block(self.runtime_context.pc()).is_ok() {
                // The block is already translated. Return
                return Ok(());
            }
            jit.new_block(EVAddr::null())?;
            // TODO: Insert instructions into block.
            let counter = self
                .processor_specification
                .borrow()
                .varnode_slice_by_string("AX")
                .unwrap();

            let inst = jinst!(counter = 0u16);
            let inst2 = jinst!(counter = counter + 1u16);
            jit.emit_jinst(&inst)?;
            jit.emit_jinst(&inst2)?;
            jit.finalize_block()?;
            // jit.chain_blocks(EVAddr::null(), EVAddr::null())?;
            Ok(())
        }
    }
}
