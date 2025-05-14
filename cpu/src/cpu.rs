//! The main cpu struct. This is the public interface that will be visible to the end user.
//! The `CPUContext` contains all the runtime information needed during emulation.
//! This includes for example the registers and a pointer to the currently active `TLB`.

use crate::arch::x86::general::{
    swap_bytes,
    swap_bytes_non_overlapping,
    swap_bytes_slice_in_out,
};
use crate::arch::x86::vector::{paddsb_slice, paddsb_slice_in_out, paddsw_slice, paddsw_slice_in_out, paddusw_slice, paddusw_slice_in_out, pavgw_slice, pavgw_slice_in_out, phaddw_slice, phaddw_slice_in_out, psubsw_slice, psubsw_slice_in_out, psubusw_slice, psubusw_slice_in_out};
use crate::jit::api::{
    CustomOpFastImplBinary,
    CustomOpFastImplUnary,
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
    BlockEndingOpDecider,
    Lifter,
    LifterConfig,
    LifterError,
};
use mogi_sleigh::spec::ProcessorSpecification;
use mogi_sleigh::spec::jir::basic_block::TargetBasicBlock;
use mogi_sleigh::spec::jir::basic_block::optimizer::OptimizerOptions;
use mogi_sleigh::spec::jir::jcode::NextBlockTarget;
use mogi_utilities::addresses::EVAddr;
use std::cell::{
    Ref,
    RefCell,
    RefMut,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::swap;
use std::num::NonZeroUsize;
use std::ops::{
    Deref,
    Range,
};
use std::process::exit;
use std::rc::Rc;
use std::time::Instant;
use thiserror::Error;
use tracing::{
    debug,
    trace,
};
use zerocopy::{
    FromBytes,
    Immutable,
    IntoBytes,
    KnownLayout,
};

macro_rules! register_binary_custom_ops {
    ($spec:expr, $jit:expr, $name:literal, $ops:expr) => {
        let op_id = $spec.custom_op_id_by_string($name).unwrap();
        $jit.register_binary_custom_op_fast_impl(op_id, $ops)
            .unwrap();
    };
}

#[derive(Error, Debug)]
pub enum StopReason {
    #[error("The jit runtime encountered an error")]
    RuntimeError(#[from] RuntimeError),
    #[error("The just in time compiler encountered an error")]
    JitError(#[from] JitError),
    #[error("The block couldn't be lifted: {0}")]
    LifterError(#[from] LifterError),
}

#[allow(dead_code)]
#[derive(Debug)]
enum MustInterpret {
    Yes,
    No,
}

#[derive(Debug, Copy, Clone)]
pub enum TranslationLimit {
    Unlimited,
    BlockCount(NonZeroUsize),
    InstructionCount(NonZeroUsize),
    Address(EVAddr),
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
    block_cache: FnvHashMap<EVAddr, TargetBasicBlock>,
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

        Self::setup_custom_ops(&mut jit, &processor_specification.borrow());

        Self {
            runtime_context,
            jit: RefCell::new(jit),
            mmu,
            processor_specification,
            block_cache: Default::default(),
        }
    }

    fn setup_custom_ops(
        jit: &mut JitT,
        spec: &ProcessorSpecification,
    ) {
        // TODO: This should be handled by some processor extension file or something similar.
        //       This file specifies the implementations for the custom ops and also if they end a block.
        //       There are three options: (1) block ending op that exits the jit with an exception
        //                                (2) custom op that is implemented via callback
        //                                (3) custom op that is implemented via a custom emitter.
        let syscall_id = spec.custom_op_id_by_string("syscall").unwrap();
        jit.mark_custom_op_as_externally_implemented(syscall_id)
            .unwrap();

        let swap_bytes_id = spec.custom_op_id_by_string("swap_bytes").unwrap();
        let swap_bytes_ops = CustomOpFastImplUnary {
            impl_8: Some(swap_bytes),
            impl_16: Some(swap_bytes),
            impl_32: Some(swap_bytes),
            impl_64: Some(swap_bytes),
            impl_128: Some(swap_bytes),
            impl_slice_non_overlapping: swap_bytes_non_overlapping,
            impl_slice_in_out: swap_bytes_slice_in_out,
        };
        jit.register_unary_custom_op_fast_impl(swap_bytes_id, swap_bytes_ops)
            .unwrap();

        register_binary_custom_ops!(spec, jit, "paddsb", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: paddsb_slice,
            impl_slice_in_out: paddsb_slice_in_out,
        });

        register_binary_custom_ops!(spec, jit, "paddsw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: paddsw_slice,
            impl_slice_in_out: paddsw_slice_in_out,
        });
        
        register_binary_custom_ops!(spec, jit, "psubsw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: psubsw_slice,
            impl_slice_in_out: psubsw_slice_in_out,
        });

        register_binary_custom_ops!(spec, jit, "paddusw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: paddusw_slice,
            impl_slice_in_out: paddusw_slice_in_out,
        });

        register_binary_custom_ops!(spec, jit, "psubusw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: psubusw_slice,
            impl_slice_in_out: psubusw_slice_in_out,
        });

        register_binary_custom_ops!(spec, jit, "pavgw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: pavgw_slice,
            impl_slice_in_out: pavgw_slice_in_out,
        });

        register_binary_custom_ops!(spec, jit, "phaddw", CustomOpFastImplBinary {
            impl_8: None,
            impl_16: None,
            impl_32: None,
            impl_64: None,
            impl_128: None,
            impl_slice_non_overlapping: phaddw_slice,
            impl_slice_in_out: phaddw_slice_in_out,
        });

    }
}

impl<JitT, MmuT> CPU for MogiCPU<JitT, MmuT>
where
    JitT: MogiJit + BlockEndingOpDecider,
    MmuT: TargetInstructionSource,
{
    fn run(
        &mut self,
        limit: RunLimit,
    ) -> Result<(), StopReason> {
        let translation_limit = match limit {
            RunLimit::Unlimited => TranslationLimit::Unlimited,
            RunLimit::InstructionCount(count) => TranslationLimit::InstructionCount(count),
            RunLimit::Address(addr) => TranslationLimit::Address(addr),
        };

        let mut executed_instructions = 0usize;

        loop {
            trace_jit!("Next block");
            self.handle_interrupt()?;
            match self.must_interpret() {
                MustInterpret::No => {
                    // This populates the jit block cache for the next instruction(s)
                    if let Err(te) = self.translate(translation_limit) {
                        trace_jit!("Translation stopped: {}", te);
                    }
                    let jit = self.jit.borrow();
                    let ip = jit.instruction_pointer(&self.runtime_context)?;
                    debug!("Executing block at {:?}", ip);
                    jit.execute(&mut self.runtime_context, ip)?;
                    executed_instructions += self
                        .block_cache
                        .get(&ip)
                        .unwrap()
                        .target_instruction_count();
                    if let Some(error) = self.runtime_context.runtime_error() {
                        Err(error)?
                    }
                }
                MustInterpret::Yes => {
                    todo!("Interpreter is currently not yet implemented")
                }
            }

            match translation_limit {
                TranslationLimit::Unlimited => {}
                // we only translate as much as necessary so we can break here
                TranslationLimit::BlockCount(_) => break,
                TranslationLimit::InstructionCount(inst_limit) => {
                    if executed_instructions >= inst_limit.into() {
                        break;
                    }
                }
                TranslationLimit::Address(addr_limit) => {
                    let ip = self
                        .jit
                        .borrow()
                        .instruction_pointer(&self.runtime_context)?;
                    if ip == addr_limit {
                        // ip can't go over addr_limit since we limit the translation
                        break;
                    }
                }
            }
        }

        Ok(())
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
    JitT: MogiJit + BlockEndingOpDecider,
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

    /// Translate the next instructions depending on `translation_limit`: -
    /// - [TranslationLimit::Unlimited]: translate as many blocks as possible -
    /// - [TranslationLimit::BlockCount]: translate at most n blocks -
    /// - [TranslationLimit::InstructionCount]: translate at most n instructions
    /// - [TranslationLimit::Address]: translate until a given address (exclusively)
    fn translate(
        &mut self,
        translation_limit: TranslationLimit,
    ) -> Result<(), StopReason> {
        let mut jit = self.jit.borrow_mut();
        let mmu = self.mmu.borrow();

        let mut target_addresses = vec![jit.instruction_pointer(&self.runtime_context)?];
        // TODO this should reside somewhere outside to prevent duplicate lifting
        // for multiple calls of translate
        let mut translated_block_count = 0usize;
        let mut translated_instr_count = 0usize;
        trace_jit!("Translation limit is {:?}", translation_limit);

        let instr_limit = match translation_limit {
            TranslationLimit::InstructionCount(instr_count) => Some(instr_count),
            _ => None,
        };

        let stop_addr = match translation_limit {
            TranslationLimit::Address(stop_addr) => Some(stop_addr),
            _ => None,
        };

        while let Some(current_addr) = target_addresses.pop() {
            debug!("Translating block at {:?}", current_addr);

            if self.block_cache.contains_key(&current_addr) {
                trace_jit!("Skipping aleady translated block");
                continue;
            }

            let config = LifterConfig::builder()
                .custom_op_decider(&*jit)
                .maybe_max_instructions(instr_limit.map(|limit| {
                    NonZeroUsize::new(limit.get().saturating_sub(translated_instr_count))
                        .expect("We should have aborted before this happens")
                }))
                .maybe_stop_addr(stop_addr)
                .build();

            let block = self.processor_specification.borrow().lift_block(
                &*mmu,
                self.runtime_context.as_mut(),
                &config,
                current_addr,
            )?;

            compile(&mut *jit, &block)?;

            process_branch_targets(&mut target_addresses, &block, translation_limit);
            process_block_fallthrough(&mut target_addresses, &block, translation_limit);

            translated_block_count += 1;
            translated_instr_count += block.target_instruction_count();
            self.block_cache.insert(current_addr, block);

            if !can_continue_translating(translation_limit, translated_instr_count, translated_block_count) {
                break;
            }
        }

        let total_instr_count: usize = self
            .block_cache
            .values()
            .map(|block| block.target_instruction_count())
            .sum();
        trace_jit!("Translated {}/{} blocks/instructions in total", self.block_cache.len(), total_instr_count);

        Ok(())
    }
}

fn add_target_address(
    target_addresses: &mut Vec<EVAddr>,
    addr: EVAddr,
    limit: TranslationLimit,
) {
    if !matches!(limit, TranslationLimit::Address(limit_addr) if addr == limit_addr) {
        target_addresses.push(addr);
    }
}

/// Processes branch targets from a basic block and adds them to the target addresses vector.
///
/// # Arguments
/// * `translation_limit` - Limit for the translation process
/// * `target_addresses` - Vector to store discovered target addresses
/// * `block` - Basic block to process branch targets from
fn process_branch_targets(
    mut target_addresses: &mut Vec<EVAddr>,
    block: &TargetBasicBlock,
    translation_limit: TranslationLimit,
) {
    // add direct branch targets
    if let Some(branch_targets) = block.branch_targets() {
        for branch_target in branch_targets {
            if let NextBlockTarget::Direct(addr) | NextBlockTarget::Return(addr) = branch_target {
                add_target_address(&mut target_addresses, *addr, translation_limit);
            }
        }
    }
}

fn process_block_fallthrough(
    mut target_addresses: &mut Vec<EVAddr>,
    block: &TargetBasicBlock,
    translation_limit: TranslationLimit,
) {
    // add fallthrough
    match block.fallthrough() {
        Some(fallthrough_addr) => {
            add_target_address(&mut target_addresses, fallthrough_addr, translation_limit);
        }
        None => {
            trace_jit!("Block has no fallthrough, skipping disassembling");
        }
    }
}

fn compile<JitT: MogiJit>(
    jit: &mut JitT,
    block: &TargetBasicBlock,
) -> Result<(), StopReason> {
    // TODO: Most likely, if the runtime context was modified, we will first need to emulate before translating
    //       more blocks?
    trace_jit!("{}", block);
    trace_jit!("Fallthrough: {:?}", block.fallthrough());
    trace_jit!("Branch targets: {:?}", block.branch_targets());

    let mut optimized_block = (*block.0).clone();
    let optimizer_options = OptimizerOptions::default().with_enable_dse(true);
    optimized_block
        .run_optimization_passes(optimizer_options)
        .unwrap();
    trace_jit!("{}", block);
    jit.compile_target_block(&block)?;
    Ok(())
}

fn can_continue_translating(
    translation_limit: TranslationLimit,
    translated_instr_count: usize,
    translated_block_count: usize,
) -> bool {
    match translation_limit {
        TranslationLimit::Unlimited => true,
        TranslationLimit::BlockCount(block_count) => {
            if translated_block_count >= block_count.into() {
                trace_jit!("Hit block translation limit");
                false
            } else {
                true
            }
        }
        TranslationLimit::InstructionCount(instr_count) => {
            if translated_instr_count >= instr_count.into() {
                trace_jit!("Hit instruction translation limit");
                false
            } else {
                true
            }
        }
        TranslationLimit::Address(_) => {
            // TODO: We need to handle this more correctly, since the target address might also be inside a block
            true
        }
    }
}

fn disassemble(
    spec: &ProcessorSpecification,
    mmu: &impl TargetInstructionSource,
    runtime_context: &mut impl ContextProvider,
    range: Range<EVAddr>,
) {
    let disassembly = spec.disassemble_range(mmu, runtime_context, range).unwrap();

    for disassembled_instr in &disassembly {
        trace_jit!("{}", disassembled_instr);
    }
}
