use std::collections::{
    BTreeMap,
    HashSet,
};
use std::mem;
use std::rc::Rc;

use blocktable::{
    BlockInfoRef,
    BlockKey,
    BlockTable,
};
use cranelift_codegen::ir::Endianness;
use exception::ExceptionCode;
use mogi_utilities::addresses::EVAddr;

use crate::core::regs::ValueSource;
use crate::core::{
    CpuCore,
    CpuSnapshot,
    Exception,
};
use crate::cpu::exit::CpuExitStatus;
use crate::cpu::injector::{
    CodeInjector,
    CodeInjectorAny,
    InjectorRef,
};
use crate::jit::runtime::{
    call_address_not_translated,
    call_block_contains_breakpoint,
    call_jit_compilation_error,
};
use crate::jit::{
    CompilationTarget,
    JitFunction,
    VmCtx,
    JIT,
};
use crate::lifter::{
    count_instructions,
    DecodeError,
    InstructionSource,
    Target,
};
use crate::pcode::PcodeDisplay;
use crate::{
    lifter,
    pcode,
    InternalError,
};

pub mod api;
pub mod blocktable;
pub mod builder;
pub mod cpuid;
mod debug;
pub mod exception;
pub mod exit;
mod injector;
pub mod interface;

const TRACE_EXEC: bool = false;

pub struct Cpu {
    core: Box<CpuCore>,
    lifter: lifter::BlockLifter,
    icount_limit: u64,
    next_timer: u64,
    interrupt_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    code: BlockTable,
    jit: JIT,
    enable_jit: bool,
    enable_recompilation: bool,
    prev_isa_mode: u8,

    /// The number of new blocks that have been compiled since the last full recompilation step.
    compiled_blocks: u64,

    injectors: Vec<Box<dyn CodeInjectorAny>>,

    /// The last time the JIT was recompiled.
    last_recompile: std::time::Instant,

    /// The offset to recompile from for block chaining.
    recompile_offset: usize,

    /// Cached pointers that are used inside of the JIT
    jit_ctx: VmCtx,

    /// Snapshots at different icounts for reverse execution.
    snapshots: Vec<Snapshot>,
    handle_exception_hook: Box<dyn FnMut(&mut CpuCore) -> Option<CpuExitStatus>>,
}

impl Drop for Cpu {
    fn drop(&mut self) {
        // Safety: After the Vm instance is destroyed there is no way to access the JIT code.
        unsafe { self.jit.reset() }
    }
}

impl Cpu {
    pub fn new(
        cpu: Box<CpuCore>,
        lifter: lifter::BlockLifter,
    ) -> Self {
        let endianness = if cpu.arch.sleigh.big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        };
        let jit = JIT::new(endianness, cpu.arch.reg_pc, cpu.enable_shadow_stack);
        Self {
            core: cpu,
            lifter,
            injectors: Vec::new(),
            icount_limit: u64::MAX,
            next_timer: 0,
            interrupt_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            code: BlockTable::default(),
            jit,
            enable_jit: true,
            enable_recompilation: false,
            prev_isa_mode: u8::MAX,

            compiled_blocks: 0,
            last_recompile: std::time::Instant::now(),
            recompile_offset: 0,
            jit_ctx: VmCtx::new(),
            snapshots: vec![],
            handle_exception_hook: Box::new(|_| None),
        }
    }

    /// Registers a [CodeInjector] in the VM which is invoked whenever the emulator lifts a new
    /// block of code.
    ///
    /// Returns a reference that can be later used to obtain mutable access to the injector using
    /// `get_injector_mut`.
    ///
    /// Note: the injector is only executed on newly lifted blocks.
    pub fn add_injector<C>(
        &mut self,
        injector: C,
    ) -> InjectorRef
    where
        C: CodeInjector + 'static,
    {
        // @todo: consider running the injector over all current blocks.
        let injector_id = self.injectors.len();
        self.injectors.push(Box::new(injector));
        injector_id
    }

    /// Gets a mutable reference previously registered injector using `id`.
    ///
    /// Note: Be wary of changing the behavior of the injector, sine it will _not_ re-executed on
    /// existing blocks.
    pub fn get_injector_mut<C>(
        &mut self,
        id: InjectorRef,
    ) -> Option<&mut C>
    where
        C: CodeInjector + 'static,
    {
        self.injectors[id].as_mut_any().downcast_mut::<C>()
    }

    /// Registers a function `hook` to called before the instruction at `addr` is executed.
    pub fn hook_address(
        &mut self,
        addr: EVAddr,
        hook: impl FnMut(&mut CpuCore, EVAddr) + 'static,
    ) {
        let hook_id = self.core.add_hook(hook);
        injector::register_instruction_hook_injector(self, vec![addr], hook_id);
    }

    /// Registers a function `hook` that called whenever any of the addresses in `addrs` are about
    /// to be executed.
    pub fn hook_many_addresses(
        &mut self,
        addrs: &[EVAddr],
        hook: impl FnMut(&mut CpuCore, EVAddr) + 'static,
    ) {
        let hook_id = self.core.add_hook(hook);
        injector::register_instruction_hook_injector(self, addrs.into(), hook_id);
    }

    /// Registers an injector that is called whenever the p-code operation `name` is translated.
    pub fn add_op_injector(
        &mut self,
        name: &str,
        injector: impl lifter::PcodeOpInjector + Sized + 'static,
    ) -> bool {
        let idx = match self.core.arch.sleigh.get_userop(name) {
            Some(idx) => idx,
            None => return false,
        };
        self.lifter.op_injectors.insert(idx, Box::new(injector));
        true
    }

    /// Runs the VM until it encounters an exit condition.
    pub fn run(&mut self) -> CpuExitStatus {
        // Run exception handling first. If we exited emulation with an exception, it will first need to be handled.
        if self.core.exception.code != ExceptionCode::None as u32 {
            match self.handle_exception() {
                CpuExitStatus::Running => {}
                exit => return exit,
            }
        }
        if self.should_recompile() && self.enable_recompilation {
            self.recompile();
        }

        if self.core.block_id == u64::MAX {
            if let Some((block, _)) = self.get_current_block() {
                self.core.block_id = block;
                self.core.block_offset = 0;
            }
        }

        self.update_timer();
        loop {
            let instructions_to_exec = self.next_timer.saturating_sub(self.core.icount);
            if instructions_to_exec > 0 {
                self.core.update_fuel(instructions_to_exec);

                self.run_block_jit();
                if self.core.exception.code == ExceptionCode::InstructionLimit as u32 {
                    self.run_block_interpreter();
                }

                // Clear fuel so `icount` is correct.
                self.core.update_fuel(0);
            } else {
                self.core.exception.code = ExceptionCode::InstructionLimit as u32;
            }

            if self.core.exception.code != ExceptionCode::None as u32 {
                match self.handle_exception() {
                    CpuExitStatus::Running => {}
                    exit => return exit,
                }
            }
        }
    }

    pub(crate) fn get_block_key(
        &self,
        vaddr: EVAddr,
    ) -> BlockKey {
        let isa_mode = self.core.isa_mode() as u64;
        BlockKey { vaddr, isa_mode }
    }

    fn handle_exception(&mut self) -> CpuExitStatus {
        if let Some(exit) = (self.handle_exception_hook)(&mut self.core) {
            return exit;
        }

        let code = ExceptionCode::from_u32(self.core.exception.code);
        tracing::trace!("{code:?}: icount={}, next_timer={}", self.core.icount, self.next_timer);
        match code {
            ExceptionCode::None | ExceptionCode::InstructionLimit => {
                if self
                    .interrupt_flag
                    .load(std::sync::atomic::Ordering::Relaxed)
                {
                    return CpuExitStatus::Interrupted;
                }
                if self.core.icount >= self.icount_limit {
                    return CpuExitStatus::InstructionLimit;
                }
                if self.code.breakpoints.contains(&self.core.read_pc()) {
                    return CpuExitStatus::Breakpoint;
                }
                self.update_timer();
                CpuExitStatus::Running
            }
            ExceptionCode::SoftwareBreakpoint => CpuExitStatus::Breakpoint,

            ExceptionCode::ExternalAddr => self.handle_external_address(self.core.exception.value.into(), u64::MAX),
            ExceptionCode::CodeNotTranslated => self.handle_code_not_translated(),
            ExceptionCode::UnimplementedOp => self.handle_unimplemented_op(),
            ExceptionCode::ShadowStackInvalid | ExceptionCode::ShadowStackOverflow => {
                // The block offset is wrong on shadow stack errors so fix it here.
                self.core.block_offset = self.code.blocks[self.core.block_id as usize]
                    .pcode
                    .instructions
                    .len() as u64;
                CpuExitStatus::UnhandledException((code, self.core.exception.value))
            }
            ExceptionCode::Halt | ExceptionCode::Sleep => CpuExitStatus::Halt,
            ExceptionCode::OutOfMemory => CpuExitStatus::OutOfMemory,
            code => CpuExitStatus::UnhandledException((code, self.core.exception.value)),
        }
    }

    fn handle_external_address(
        &mut self,
        addr: EVAddr,
        prev_block: u64,
    ) -> CpuExitStatus {
        self.core.write_pc(addr);

        let key = self.get_block_key(addr);
        match self.code.map.get(&key) {
            Some(group) => {
                self.core.block_id = group.blocks.0 as u64;
                self.core.block_offset = 0;
                CpuExitStatus::Running
            }
            None => {
                self.core.block_id = prev_block;
                self.handle_code_not_translated()
            }
        }
    }

    #[cold]
    fn handle_code_not_translated(&mut self) -> CpuExitStatus {
        let pc = self.core.read_pc();
        // Check for internal errors (e.g. if code map is invalid).
        let key = self.get_block_key(pc);
        if self.code.map.contains_key(&key) {
            tracing::error!(
                "Internal error: `self.code.map` is invalid, \
                        expected block at {key:x?} to be missing",
            );
            return CpuExitStatus::UnhandledException((
                ExceptionCode::InternalError,
                InternalError::CorruptedBlockMap as u64,
            ));
        }

        match self.lift(pc.into()) {
            Ok(group) => {
                self.core.block_id = group.blocks.0 as u64;
                self.core.block_offset = 0;
                CpuExitStatus::Running
            }
            Err(e) => {
                tracing::trace!("DecodeError at {pc:?}: {e:?}");
                self.core.exception = Exception::new(ExceptionCode::from(e), pc.into());
                if self.core.icount >= self.icount_limit {
                    return CpuExitStatus::InstructionLimit;
                }
                self.handle_exception()
            }
        }
    }

    /// Handles the case where we encounter an unhandled user-defined pcode operation during
    /// execution.
    ///
    /// Note: Handlers for many of the commonly used operations are mapped in
    /// icicle-cpu/src/exec/helpers.rs.
    #[cold]
    fn handle_unimplemented_op(&mut self) -> CpuExitStatus {
        let block = &self.code.blocks[self.core.block_id as usize];
        let stmt = block.pcode.instructions[self.core.block_offset as usize];
        tracing::error!(
            "[{:?}] unknown pcode operation: {}",
            self.core.read_pc(),
            stmt.display(&self.core.arch.sleigh)
        );
        CpuExitStatus::UnhandledException((ExceptionCode::UnimplementedOp, self.core.exception.value))
    }

    #[cold]
    #[inline(never)]
    fn corrupted_block_map(
        &mut self,
        id: u64,
    ) {
        self.core.exception.code = ExceptionCode::InternalError as u32;
        self.core.exception.value = InternalError::CorruptedBlockMap as u64;
        let pc = self.core.read_pc();
        eprintln!("Block map corrupted at: pc={pc:?} id={id}\n{}", std::backtrace::Backtrace::force_capture());
    }

    fn update_timer(&mut self) {
        /// The number of instructions to wait before checking `vm.interrupt_flag`. Should be set
        /// quite high since it causes a full VM exit to check.
        #[cfg(debug_assertions)]
        const CHECK_FOR_INTERRUPT_FLAG_TIMER: u64 = 0x1_0000;
        #[cfg(not(debug_assertions))]
        const CHECK_FOR_INTERRUPT_FLAG_TIMER: u64 = 0x10_0000;

        let user_exit = self.icount_limit;
        let env_exit = u64::MAX; // TODO: Reenable env hook ? self.env.next_timer();
        self.next_timer = user_exit
            .min(env_exit)
            .min(CHECK_FOR_INTERRUPT_FLAG_TIMER + self.core.icount);
    }

    #[cold]
    fn run_block_interpreter(&mut self) {
        self.core.exception.clear();

        let (mut block_id, mut offset) = match self.get_current_block() {
            Some(value) => value,
            None => {
                self.core.exception.code = ExceptionCode::CodeNotTranslated as u32;
                self.core.exception.value = self.core.read_pc().into();
                return;
            }
        };
        if TRACE_EXEC {
            print_interpreter_enter(self, block_id, offset);
        }
        self.core.block_offset = 0;
        loop {
            let block = match self.code.blocks.get(block_id as usize) {
                Some(block) => block,
                None => return self.corrupted_block_map(block_id),
            };

            if block.has_breakpoint() {
                // Determine how many steps to execute before we hit the first breakpoint in this
                // block.
                for (i, inst) in block.pcode.instructions[offset as usize..]
                    .iter()
                    .filter(|inst| matches!(inst.op, pcode::Op::InstructionMarker))
                    .enumerate()
                {
                    if self
                        .code
                        .breakpoints
                        .contains(&EVAddr::from(inst.inputs.first().as_u64()))
                    {
                        self.core
                            .update_fuel(self.core.fuel.remaining.min(i as u64));
                        break;
                    }
                }
            }

            // Safety: we validate each block as part of `lift`.
            unsafe {
                if let Some(offset) = self
                    .core
                    .interpret_block_unchecked(&block.pcode, offset as usize)
                {
                    // Keep track of the offset where the CPU exited from.
                    self.core.block_id = block_id;
                    self.core.block_offset = offset as u64;
                    break;
                }
            }

            match self.core.block_exit(block.exit) {
                Target::Internal(id) => {
                    block_id = id as u64;
                    offset = 0;
                }
                Target::External(addr) => {
                    let addr: u64 = self.core.read_dynamic(addr).zxt();
                    let addr = EVAddr::from(addr);
                    self.core.write_pc(addr);

                    match self.code.map.get(&self.get_block_key(addr)) {
                        Some(group) => {
                            self.core.block_id = group.blocks.0 as u64;
                            self.core.block_offset = 0;
                        }
                        None => {
                            self.core.block_id = block_id;
                            self.core.exception.code = ExceptionCode::CodeNotTranslated as u32;
                            self.core.exception.value = addr.into();
                        }
                    }

                    // We always break here even if we know which block to execute next to allow the
                    // emulator to re-enter the JIT.
                    break;
                }
                Target::Invalid(e, addr) => {
                    tracing::debug!(
                        "End of block has invalid target\n{}\n{e:?} @ {addr:#x}, PC: {:?}",
                        debug::debug_addr(self, block.start).unwrap(),
                        self.core.read_pc()
                    );

                    // Synchronize the RIP (this is necessary if an invalid instruction occurs in
                    // the middle of a block).
                    self.core.write_pc(addr.into());

                    // Raise the exception
                    self.core.exception = Exception::new(ExceptionCode::from(e), addr);

                    break;
                }
            }
        }

        if TRACE_EXEC {
            print_interpreter_exit(self);
        }
    }

    #[inline]
    fn can_enter_jit(&mut self) -> bool {
        // Avoid entering the JIT if: the jit is disabled, we are in the middle of a block, or we
        // are at the start of an internal block.
        self.core.block_offset == 0
            && self
                .code
                .blocks
                .get(self.core.block_id as usize)
                .map_or(false, |x| x.entry.is_some())
            && self.enable_jit
    }

    fn run_block_jit(&mut self) {
        if !self.can_enter_jit() {
            self.core.exception.code = ExceptionCode::InstructionLimit as u32;
            self.core.exception.value = 0;
            return;
        }

        self.core.exception.clear();

        // Update pointers stored in the JIT.
        // @todo: optimize: this doesn't need to be done every time we enter the JIT.
        self.jit_ctx.tlb_ptr = self.core.mem.borrow_mut().tlb_ptr();
        for (dst, src) in self
            .jit_ctx
            .tracer_mem
            .iter_mut()
            .zip(self.core.trace.storage_ptr())
        {
            *dst = src;
        }

        for (dst, hook) in self.jit_ctx.hooks.iter_mut().zip(self.core.get_hooks()) {
            (dst.fn_ptr, dst.data_ptr) = hook.get_ptr();
        }

        let mut next_addr = self.core.read_pc();
        if TRACE_EXEC {
            print_jit_enter(self, next_addr);
        }
        while self.core.exception.code == ExceptionCode::None as u32 {
            let jit_func = match self.jit.lookup_fast(next_addr) {
                Some(func) => {
                    self.jit.jit_hit += 1;
                    func
                }
                None => self.get_or_compile_jit_block(next_addr),
            };

            // Safety: the JIT must generate code that is safe to execute.
            unsafe {
                next_addr = jit_func(self.core.as_mut(), &mut self.jit_ctx, next_addr);
            }
        }

        if self.core.block_offset != 0 {
            self.jit_exit_before_end_of_block();
        }

        if TRACE_EXEC {
            print_jit_exit(self, next_addr);
        }
    }

    #[cold]
    fn jit_exit_before_end_of_block(&mut self) {
        // Since we exited before we reached the end of a block, we need to check how many
        // instructions we executed in the block to adjust the consumed fuel.
        let block = match self.code.blocks.get(self.core.block_id as usize) {
            Some(block) => block,
            None => return self.corrupted_block_map(self.core.block_id),
        };
        let executed_instructions = count_instructions(&block.pcode.instructions[..self.core.block_offset as usize]);
        self.core.fuel.remaining -= executed_instructions as u64;
    }

    /// Return the currently active block and offset. If no block is active, retrieve the next block
    /// based on the current program counter.
    pub fn get_current_block(&self) -> Option<(u64, u64)> {
        match self.core.block_id != u64::MAX {
            true => Some((self.core.block_id, self.core.block_offset)),
            false => {
                let key = self.get_block_key(self.core.read_pc());
                let id = self.code.map.get(&key).map(|group| group.blocks.0)?;
                Some((id as u64, 0))
            }
        }
    }

    // pub fn reset(&mut self) {
    //     self.core.reset();
    //     self.core.mem.borrow_mut().clear();
    //     self.code.flush_code();
    //     if self.enable_jit {
    //         self.jit.clear();
    //     }
    //     self.prev_isa_mode = u8::MAX;
    // }

    pub fn get_disasm(
        &self,
        addr: EVAddr,
    ) -> Option<&str> {
        self.code.disasm.get(&addr).map(|s| s.as_str())
    }

    pub fn get_block_info(
        &self,
        addr: EVAddr,
    ) -> Option<BlockInfoRef> {
        let key = self.get_block_key(addr);
        self.code.get_info(key)
    }

    pub fn lift(
        &mut self,
        addr: EVAddr,
    ) -> Result<lifter::BlockGroup, DecodeError> {
        self.update_context();

        let mut ctx = lifter::Context::new(&mut *self.core, &mut self.code, addr);
        let group = self.lifter.lift_block(&mut ctx)?;

        // Add breakpoints to the lifted code.
        if self.code.breakpoints.len() > 0 {
            for block in &mut self.code.blocks[group.range()] {
                for inst in &block.pcode.instructions {
                    if matches!(inst.op, pcode::Op::InstructionMarker)
                        && self
                            .code
                            .breakpoints
                            .contains(&EVAddr::from(inst.inputs.first().as_u64()))
                    {
                        block.breakpoints += 1;
                    }
                }
            }
        }

        for injector in &mut self.injectors {
            injector.inject(&mut self.core, &group, &mut self.code);
        }

        self.code.modified.extend(group.range());

        // Validate that all modified code is valid, and invalidate any jitted code that is now
        // modified.
        for id in self.code.modified.drain() {
            let block = &mut self.code.blocks[id];
            for inst in &block.pcode.instructions {
                if !self.core.validate(inst) {
                    panic!(
                        "block {:?} contains invalid instruction {} ({:?})",
                        block.start,
                        inst.display(&self.core.arch.sleigh),
                        inst,
                    );
                }
            }
            self.jit.invalidate(id);
        }

        let key = self.get_block_key(addr);
        self.code.map.insert(key, group);

        tracing::trace!(
            "lifted: {key:x?} => {}",
            group
                .to_string(&self.code.blocks, &self.core.arch.sleigh, false)
                .unwrap()
        );

        Ok(group)
    }

    fn update_context(&mut self) {
        // Use the context from the last block.
        if let Some(block) = self.code.blocks.get(self.core.block_id as usize) {
            self.lifter.set_context(block.context);
        }

        // Check for ISA mode changes.
        let isa_mode = self.core.isa_mode();
        if self.prev_isa_mode != isa_mode {
            tracing::debug!("ISA mode change {} -> {isa_mode}", self.prev_isa_mode);
            self.jit.clear_fast_lookup();
            self.prev_isa_mode = isa_mode;
            match self.core.arch.isa_mode_context.get(isa_mode as usize) {
                Some(ctx) => self.lifter.set_context(*ctx),
                None => self.invalid_isa_mode(),
            }
        }
    }

    #[inline(never)]
    #[cold]
    fn invalid_isa_mode(&mut self) {
        tracing::error!("Unknown or unsupported ISA mode: {}", self.prev_isa_mode);
        self.core.exception.code = ExceptionCode::InternalError as u32;
        self.core.exception.value = InternalError::CorruptedBlockMap as u64;
    }

    #[inline(never)]
    #[cold]
    fn get_or_compile_jit_block(
        &mut self,
        addr: EVAddr,
    ) -> JitFunction {
        // Try to find the block corresponding to the target address.
        let key = self.get_block_key(addr);
        let group = match self.code.map.get(&key) {
            Some(group) => group,
            None => return call_address_not_translated(),
        };

        if self.prev_isa_mode != key.isa_mode as u8 {
            // ISA mode has changed so we need to prevent addresses from the previous ISA mode from
            // being included in the fast look up table (which doesn't check the ISA mode).
            self.prev_isa_mode = key.isa_mode as u8;
            self.jit.clear_fast_lookup();
        }

        self.core.block_id = group.blocks.0 as u64;
        self.jit.jit_miss += 1;

        // Check if there is a breakpoint set on any of the blocks in the group.
        for block in &self.code.blocks[group.range()] {
            if block.breakpoints > 0 {
                return call_block_contains_breakpoint();
            }
        }

        // See if we already have compile the block, but it was inactive.
        if let Some(&fn_ptr) = self.jit.entry_points.get(&addr) {
            self.jit.add_fast_lookup(addr, fn_ptr);
            return fn_ptr;
        }

        // The block needs to be compiled
        self.compiled_blocks += 1;
        tracing::trace!("compile_block: key={key:x?} ({} new)", self.compiled_blocks);
        let blocks = group.range().collect::<Vec<_>>();
        let target = CompilationTarget::new(&self.code.blocks, &blocks);
        if let Err(e) = self.jit.compile(&target) {
            tracing::error!("JIT compilation failed: {:?}", e);
            return call_jit_compilation_error();
        }

        let fn_ptr = self.jit.entry_points[&addr];
        self.jit.add_fast_lookup(addr, fn_ptr);

        fn_ptr
    }

    pub fn should_recompile(&self) -> bool {
        self.compiled_blocks > 10 && self.last_recompile.elapsed().as_secs() > 60
    }

    /// Attempt to recompile all code in the code-cache, potentially improving performance by
    /// increasing cache locality, and allowing for block-chaining.
    #[inline(never)]
    #[cold]
    pub fn recompile(&mut self) {
        let start = std::time::Instant::now();

        if self.jit.should_purge() {
            // Safety: The only way functions from the JIT can be executed is through the Vm struct
            // which we have a unique reference to.
            unsafe { self.jit.reset() }
            self.recompile_offset = 0;
        }

        let mut visited: HashSet<usize> = HashSet::new();

        // Attempt to group blocks that share direct jumps.
        //
        // @fixme: currently the group is done based on the order the blocks were first hit, this is
        // not optimal.
        let mut stack = vec![];
        let mut compilation_group = vec![];

        // @fixme: this is broken if a jump performs a context switch.
        let isa_mode = self.core.isa_mode() as u64;

        for (i, block) in self
            .code
            .blocks
            .iter()
            .enumerate()
            .skip(self.recompile_offset)
        {
            let entry = match block.entry {
                Some(entry) => entry,
                // Internal blocks are only accessible from another block that has a valid entry
                // address. We can skip them here, since they will be compiled with the entry block.
                None => continue,
            };

            stack.push((i, block));
            while let Some((id, block)) = stack.pop() {
                // Avoid compiling blocks that are compiled as part of other block groups.
                if !visited.insert(id) || id < self.recompile_offset {
                    continue;
                }
                compilation_group.push(id);

                let mut add_target = |target: &lifter::Target| match target {
                    Target::Internal(id) => stack.push((*id, &self.code.blocks[*id])),
                    Target::External(pcode::Value::Const(addr, _)) => {
                        let key = BlockKey {
                            vaddr: (*addr).into(),
                            isa_mode,
                        };
                        if let Some(group) = self.code.map.get(&key) {
                            let entry_block = group.blocks.0;
                            stack.push((entry_block, &self.code.blocks[entry_block]));
                        }
                    }
                    _ => {}
                };

                match block.exit {
                    lifter::BlockExit::Jump { target } => add_target(&target),
                    lifter::BlockExit::Branch {
                        target, fallthrough, ..
                    } => {
                        add_target(&target);
                        add_target(&fallthrough);
                    }
                    lifter::BlockExit::Call { .. } | lifter::BlockExit::Return { .. } => {}
                }
            }

            if !compilation_group.is_empty() {
                tracing::trace!("[{entry:?}] compiled: {compilation_group:?}");
                let target = CompilationTarget::new(&self.code.blocks, &compilation_group);
                if let Err(e) = self.jit.compile(&target) {
                    tracing::error!("JIT compilation failed: {:?}", e);
                }
                compilation_group.clear();
            }
        }

        tracing::info!(
            "Recompiled {} blocks in {:?} ({} entrypoints now dead)",
            self.code.blocks.len() - self.recompile_offset,
            start.elapsed(),
            self.jit.dead,
        );

        self.compiled_blocks = 0;
        self.recompile_offset = self.code.blocks.len();
        self.last_recompile = std::time::Instant::now();
    }
}

#[inline(never)]
fn print_interpreter_enter(
    vm: &mut Cpu,
    block_id: u64,
    offset: u64,
) {
    let addr = vm.code.address_of(block_id, offset);
    eprintln!("interpreter_enter: next_addr={addr:#x}, block.id={block_id}, block.offset={offset}");
}

fn print_interpreter_exit(vm: &mut Cpu) {
    eprintln!(
        "interpreter_exit: (code={:?}, value={:x}), block={}, offset={}, fuel={}, icount={}",
        ExceptionCode::from_u32(vm.core.exception.code),
        vm.core.exception.value,
        vm.core.block_id,
        vm.core.block_offset,
        vm.core.fuel.remaining,
        vm.core.icount(),
    );
}

#[inline(never)]
fn print_jit_enter(
    vm: &mut Cpu,
    next_addr: EVAddr,
) {
    eprintln!("jit_enter: next_addr={next_addr:?}, fuel={}, icount={}", vm.core.fuel.remaining, vm.core.icount());
}

#[inline(never)]
fn print_jit_exit(
    vm: &mut Cpu,
    next_addr: EVAddr,
) {
    eprintln!(
        "jit_exit: (code={:?}, value={:x}), block={}, offset={}, next_addr={next_addr:?}, fuel={}, icount={}",
        ExceptionCode::from_u32(vm.core.exception.code),
        vm.core.exception.value,
        vm.core.block_id,
        vm.core.block_offset,
        vm.core.fuel.remaining,
        vm.core.icount(),
    );
}

// Helper functions
impl Cpu {
    pub fn step(
        &mut self,
        steps: u64,
    ) -> CpuExitStatus {
        let old_limit = self.icount_limit;

        self.icount_limit = self.core.icount.saturating_add(steps);
        let exit = self.run();

        self.icount_limit = old_limit;
        exit
    }

    // pub fn run_until(
    //     &mut self,
    //     addr: u64,
    // ) -> CpuExitStatus {
    //     let added_bp = self.add_breakpoint(addr);
    //     let exit = self.run();
    //     if added_bp {
    //         self.remove_breakpoint(addr);
    //     }
    //     exit
    // }
    //
    // /// Adds a breakpoint at `addr`.
    // ///
    // /// Returns a boolean representing whether a new breakpoint was added.
    // pub fn add_breakpoint(
    //     &mut self,
    //     addr: u64,
    // ) -> bool {
    //     if !self.code.breakpoints.insert(addr) {
    //         // There is already a breakpoint at the target address.
    //         return false;
    //     }
    //
    //     for block in self
    //         .code
    //         .blocks
    //         .iter_mut()
    //         .filter(|x| x.start <= addr && addr < x.end)
    //     {
    //         block.breakpoints += 1;
    //     }
    //
    //     // Make sure that any JIT blocks containing this address are removed from fast lookup.
    //     self.jit.remove_fast_lookup(addr);
    //
    //     true
    // }

    /// Removes the breakpoint at `addr`.
    ///
    /// Returns a boolean representing whether a breakpoint was remove.
    // pub fn remove_breakpoint(
    //     &mut self,
    //     addr: u64,
    // ) -> bool {
    //     if !self.code.breakpoints.remove(&addr) {
    //         // The breakpoint we are trying to remove does not exist.
    //         return false;
    //     }
    //
    //     for block in self
    //         .code
    //         .blocks
    //         .iter_mut()
    //         .filter(|x| x.start <= addr && addr < x.end)
    //     {
    //         block.breakpoints -= 1;
    //     }
    //
    //     true
    // }

    // pub fn toggle_breakpoint(
    //     &mut self,
    //     addr: u64,
    // ) -> bool {
    //     if self.add_breakpoint(addr) {
    //         return true;
    //     }
    //     self.remove_breakpoint(addr);
    //     false
    // }

    pub fn get_callstack(&self) -> Vec<EVAddr> {
        let pc = self.core.read_pc();
        self.core
            .shadow_stack
            .as_slice()
            .iter()
            .map(|entry| entry.addr)
            .chain(Some(pc))
            .collect()
    }

    // Like `get_callstack` returns callstack based on heuristics or debug info if shadow stack is
    // not available.
    // pub fn get_debug_callstack(&mut self) -> Vec<u64> {
    //     if self.cpu.enable_shadow_stack {
    //         return self.get_callstack();
    //     }
    //
    //     // Use experimental backtrace collector using debug info or frame pointer.
    //     // @fixme: we should prefer frame pointer base backtrace if available.
    //     callstack_from_debug_info(self)
    //         .or_else(|| debug::callstack_from_frame_pointer(self))
    //         .unwrap_or_else(|| vec![self.cpu.read_pc()])
    // }
}

pub struct Snapshot {
    pub core: Box<CpuSnapshot>,
}
