//! The main cpu struct. This is the public interface that will be visible to the end user.
//! The `CPUContext` contains all the runtime information needed during emulation.
//! This includes for example the registers and a pointer to the currently active `TLB`.

use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use crate::jit::api::{
    JITPathChoice,
    JITTaskRuntimeContext,
    JitError,
    MogiJit,
    RuntimeError,
};
use crate::new_cpu_trait::{
    CPU,
    CPUResult,
    RunLimit,
};
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_system::components::mmu::MMU;
use mogi_utilities::addresses::EVAddr;
use thiserror::Error;
use zerocopy::{
    FromBytes,
    Immutable,
    IntoBytes,
    KnownLayout,
};

#[derive(Error, Debug)]
pub enum StopReason {
    #[error("The cpu encountered the halt instruction")]
    Halt,
    #[error("The user aborted execution")]
    UserAbort,
    #[error("An interrupt was encountered")]
    Interrupt,
    #[error("The jit runtime encountered an error")]
    RuntimeError(#[from] RuntimeError),
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
pub struct MogiCPU<JitT, MmuT, LifterT>
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
    mmu: Rc<RefCell<MmuT>>,
    lifter: LifterT,
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    mir_cache: (),
}

impl<JitT, MmuT, LifterT> MogiCPU<JitT, MmuT, LifterT>
where
    JitT: MogiJit,
    MmuT: MMU,
{
    pub fn new(
        mut jit: JitT,
        processor_specification: Rc<RefCell<ProcessorSpecification>>,
        mmu: Rc<RefCell<MmuT>>,
        lifter: LifterT,
    ) -> Self {
        Self {
            runtime_context: Box::new(jit.new_runtime_context()),
            jit: RefCell::new(jit),
            mmu,
            lifter,
            processor_specification,
            mir_cache: (),
        }
    }
}

impl<JitT, MmuT, LifterT> CPU for MogiCPU<JitT, MmuT, LifterT>
where
    JitT: MogiJit,
{
    fn run(
        &mut self,
        limit: RunLimit,
    ) -> Result<(), StopReason> {
        let RunLimit::Unlimited = limit else {
            todo!("RunLimit is currently not yet implemented")
        };
        loop {
            self.handle_interrupt()?;
            // This populates the jit block cache for the next instruction(s)
            self.translate_blocks()?;
            match self.must_interpret() {
                MustInterpret::No => {
                    let jit = self.jit.borrow();
                    let ip = jit.instruction_pointer(&self.runtime_context)?;
                    jit.execute(&mut self.runtime_context, ip)?;
                    if let Some(error) = self.runtime_context.runtime_error() {
                        Err(error)?
                    }
                }
                MustInterpret::Yes => {
                    todo!("Interpreter is currently not yet implemented")
                }
            }
        }
    }

    fn register<T>(
        &self,
        name: &str,
    ) -> CPUResult<&T>
    where
        T: FromBytes + KnownLayout + Immutable + IntoBytes,
    {
        let varnode_id = self
            .processor_specification
            .borrow()
            .varnode_id_by_string(name)
            .unwrap();
        let jit = self.jit.borrow();
        let data = jit.varnode(&self.runtime_context, varnode_id).unwrap();
        Ok(T::ref_from_bytes(data).unwrap())
    }

    fn register_mut<T>(
        &mut self,
        name: &str,
    ) -> CPUResult<&mut T>
    where
        T: FromBytes + KnownLayout + Immutable + IntoBytes,
    {
        let varnode_id = self
            .processor_specification
            .borrow()
            .varnode_id_by_string(name)
            .unwrap();
        let jit = self.jit.borrow();
        let data = jit
            .varnode_mut(&mut self.runtime_context, varnode_id)
            .unwrap();
        Ok(T::mut_from_bytes(data).unwrap())
    }
}

impl<JitT, MmuT, LifterT> MogiCPU<JitT, MmuT, LifterT>
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
            if jit.is_cached(jit.instruction_pointer(&self.runtime_context)?) {
                // The block is already translated. Return
                return Ok(());
            }
            jit.new_target_block(EVAddr::null(), Some(EVAddr::null()))?;
            // TODO: Insert instructions into block.

            jit.finalize_block()?;
            jit.chain_blocks(EVAddr::null(), EVAddr::null(), JITPathChoice::Main)?;
            Ok(())
        }
    }
}
