//! The main cpu struct. This is the public interface that will be visible to the end user.
//! The `CPUContext` contains all the runtime information needed during emulation.
//! This includes for example the registers and a pointer to the currently active `TLB`.

use crate::jit::api::{
    JITPathChoice,
    JITTaskRuntimeContext,
    JitError,
    MogiJit,
    RuntimeError,
};
use crate::jit::trace_jit;
use crate::new_cpu_trait::{
    CPU,
    CPUResult,
    RunLimit,
};
use fnv::FnvHashMap;
use mogi_sleigh::disassembler::Disassembler;
use mogi_sleigh::lifter::context_provider::ContextProvider;
use mogi_sleigh::lifter::source::TargetInstructionSource;
use mogi_sleigh::lifter::{
    Lifter,
    LifterError,
};
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use mogi_sleigh::spec::jir::jcode::NextBlockTarget;
use mogi_utilities::addresses::EVAddr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use tracing::trace;
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
    #[error("The block couldn't be lifted")]
    LifterError(#[from] LifterError),
}

#[allow(dead_code)]
#[derive(Debug)]
enum MustInterpret {
    Yes,
    No,
}

#[derive(Debug)]
pub enum TranslationLimit {
    Unlimited,
    BlockCount(usize),
    InstructionCount(usize),
}

#[repr(C)]
pub struct MogiCPU<JitT, MmuT>
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
    processor_specification: Rc<RefCell<ProcessorSpecification>>,
    mir_cache: (),
}

impl<JitT, MmuT> MogiCPU<JitT, MmuT>
where
    JitT: MogiJit,
{
    pub fn new(
        mut jit: JitT,
        processor_specification: Rc<RefCell<ProcessorSpecification>>,
        mmu: Rc<RefCell<MmuT>>,
    ) -> Self {
        // TODO: Make an api for the runtime configuration. We probably want some kind of json file for this?
        let runtime_context = {
            let spec = processor_specification.borrow();
            let mut runtime_context = Box::new(jit.new_runtime_context());
            runtime_context.set(&*spec, "longMode", 1);
            runtime_context.set(&*spec, "addrsize", 2);
            runtime_context.set(&*spec, "opsize", 1);
            runtime_context
        };

        Self {
            runtime_context,
            jit: RefCell::new(jit),
            mmu,
            processor_specification,
            mir_cache: (),
        }
    }
}

impl<JitT, MmuT> CPU for MogiCPU<JitT, MmuT>
where
    JitT: MogiJit,
    MmuT: TargetInstructionSource,
{
    fn run(
        &mut self,
        limit: RunLimit,
    ) -> Result<(), StopReason> {
        let RunLimit::Unlimited = limit else {
            todo!("RunLimit is currently not yet implemented")
        };
        loop {
            trace_jit!("Next block");
            self.handle_interrupt()?;
            match self.must_interpret() {
                MustInterpret::No => {
                    // This populates the jit block cache for the next instruction(s)
                    self.translate_blocks(TranslationLimit::BlockCount(5))?;
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

impl<JitT, MmuT> MogiCPU<JitT, MmuT>
where
    JitT: MogiJit,
    MmuT: TargetInstructionSource,
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
    fn translate_blocks(
        &mut self,
        translation_limit: TranslationLimit,
    ) -> Result<(), StopReason> {
        // // TODO: Return translate error instead of stop reason?
        // // Load target instructions into mir cache
        // let new_blocks = self.mir_cache.populate(self.context.pc());
        // // Translate to host blocks
        // let blocks = new_blocks.translate();
        // self.jit_cache.populate(blocks);

        {
            // TODO: Remove demo block
            let mut jit = self.jit.borrow_mut();
            let mmu = self.mmu.borrow();

            let mut target_addresses = vec![jit.instruction_pointer(&self.runtime_context)?];
            let mut block_cache: FnvHashMap<EVAddr, TargetBasicBlock> = Default::default();
            let mut translated_block_count = 0;
            let mut translated_instr_count = 0;
            trace_jit!("Translation limit is {:?}", translation_limit);

            while let Some(current_addr) = target_addresses.pop() {
                trace_jit!("Translating block at {:?}", current_addr);

                if block_cache.contains_key(&current_addr) {
                    trace_jit!("Skipping aleady translated block");
                    continue;
                }

                let block = self.processor_specification.borrow().lift_block(
                    &*mmu,
                    self.runtime_context.as_mut(),
                    current_addr,
                )?;
                // let disassembly = self.processor_specification.borrow().disassemble_block(&*mmu, self.runtime_context.as_mut(), current_addr).unwrap();
                // trace_jit!("{:#?}", disassembly);

                // TODO: Most likely, if the runtime context was modified, we will first need to emulate before translating
                //       more blocks?
                trace_jit!("{}", block);
                trace_jit!("Fallthrough: {:?}", block.fallthrough());
                trace_jit!("Branch targets: {:?}", block.branch_targets());

                if let Some(branch_targets) = block.branch_targets() {
                    for branch_target in branch_targets {
                        if let NextBlockTarget::Direct(target_addr) = branch_target {
                            target_addresses.push(*target_addr);
                        }
                    }
                }

                translated_instr_count += block.instruction_count();
                translated_block_count += 1;

                jit.compile_target_block(&block)?;

                block_cache.insert(current_addr, block);

                match translation_limit {
                    TranslationLimit::Unlimited => {}
                    TranslationLimit::BlockCount(block_count) => {
                        if translated_block_count > block_count {
                            trace_jit!("Hit block translation limit");
                            break;
                        }
                    }
                    TranslationLimit::InstructionCount(instr_count) => {
                        if translated_instr_count > instr_count {
                            trace_jit!("Hit instruction translation limit");
                            break;
                        }
                    }
                }
            }

            let total_instr_count: usize = block_cache
                .values()
                .map(|block| block.instruction_count())
                .sum();
            trace_jit!("Translated {}/{} blocks/instructions in total", block_cache.len(), total_instr_count);

            Ok(())
        }
    }
}
