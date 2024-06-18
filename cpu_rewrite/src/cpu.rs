//! The main cpu struct. This is the public interface that will be visible to the end user.
//! The `CPUContext` contains all the runtime information needed during emulation.
//! This includes for example the registers and a pointer to the currently active `TLB`.

use crate::cpu_context::CPUContext;
use crate::jit::api::{
    JitError,
    MogiJit,
};
use mogi_utilities::addresses::EVAddr;
use std::cell::RefCell;
use std::ffi::c_void;
use thiserror::Error;

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

enum MustInterpret {
    Yes,
    No,
}

#[repr(C)]
pub struct CPU<JitT>
where
    JitT: MogiJit,
{
    context: CPUContext,
    /// The jit is behind a refcell, so we can guarantee that the cpu can be mutably accessed by the jited code,
    /// but the jit itself may not be modified during the codes runtime.
    jit: RefCell<JitT>,
    mmu: (),
    mir_cache: (),
}

impl<JitT> CPU<JitT>
where
    JitT: MogiJit,
{
    pub fn new(jit: JitT) -> Self {
        Self {
            context: CPUContext::new(),
            jit: RefCell::new(jit),
            mmu: (),
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
                    let context = self as *mut _ as *mut c_void;
                    let jit = self.jit.borrow();
                    let block = jit.fetch_block(self.context.pc())?;
                    jit.prelude()?(context, block);
                    // self.prelude.run_block(block);
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
            let block = jit.new_block(EVAddr::null())?;
            jit.activate_block(block)?;
            jit.emit_jmp_epilogue()?;
            jit.finalize_block(block)?;
            jit.chain_blocks(EVAddr::null(), EVAddr::null())?;
            Ok(())
        }
    }
}
