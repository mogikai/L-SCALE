use crate::jit::api::{
    JITResult,
    JITTaskRuntimeContext,
    JitError,
    TlbMissHelper,
};
use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::registers::general_purpose::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::x64emitter::helpers::reg;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::tlb::entry::TLBEntry;
use crate::tlb::{
    AccessType,
    PageIndex,
    PageTag,
};
use mogi_mem::physical::page::PageData;
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_system::components::mmu::PageMetaInfo;
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use mogi_utilities::index_map::MapKey;
use std::hint::black_box;
use std::marker::PhantomData;

impl X64AllocRequirements {
    pub fn load_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        addr: JIRValue,
        _space_id: SpaceId,
    ) {
        // We always need the output
        Self::require_large_int_write_any(requirement, output);

        // The address might be in a slice, so request it if it is not const.
        match addr {
            JIRValue::Slice(addr) => {
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::DX.into_host_reg(), addr));
                *requirement.required_temps_mut() = TempRequest::new().int(1);
            }
            JIRValue::Const(_) => {
                *requirement.required_temps_mut() = TempRequest::new().int(2);
            }
        }
    }

    pub fn store_requirements(
        requirement: &mut AllocRequirement,
        addr: JIRValue,
        _space_id: SpaceId,
        value: JIRValue,
    ) {
        // always at least 1 temp reg for tlb_lookup
        let mut required_temp_count = 1;

        // Both address and value need to be stored in slices, so we allocate enough
        // temporaries (either 1 or 2) if one of them is a const.
        match addr {
            JIRValue::Slice(addr) => {
                // addr slice => read from DX
                requirement.add_request(OperandRequest::int_read_fixed_reg(GPRegister::DX.into_host_reg(), addr));
            }
            JIRValue::Const(addr) => {
                // addr const => add temp reg
                required_temp_count = 2;
            }
        }

        match value {
            JIRValue::Slice(value) => {
                // value slice => read from registers
                Self::require_large_int_read_any(requirement, value);
            }
            JIRValue::Const(value) => {
                if value.size() > 8 {
                    // value const with size > 8 => add temp reg
                    required_temp_count = 2;
                }
            }
        }

        *requirement.required_temps_mut() = TempRequest::new().int(required_temp_count);
    }
}

impl X64Emitter<'_> {
    pub fn emit_load(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        addr: JIRValue,
        space_id: SpaceId,
    ) -> JITResult {
        if space_id == self.processor_spec.const_space() {
            let output_reg = reg!(self.allocations, inst, 0);
            let JIRValue::Const(addr) = addr else {
                panic!("Load from const space must have a constant address")
            };
            // Const address is interpreted as the value at that address.
            // TODO: Perform these const loads in the optimizer instead and propagate the constants.
            return self.load_const(output_reg, addr);
        }
        // Address is the second operand
        let addr_index = match output.size() {
            ..=8 => 1,
            16 => 2,
            size => todo!("Unsupported size {size}"),
        };

        let addr_reg = self.emit_tlb_lookup(inst, addr_index, addr, space_id, output.size(), AccessType::Load)?;
        self.emit_host_load(inst, addr_reg, output)
    }

    pub fn emit_store(
        &mut self,
        inst: InstId,
        addr: JIRValue,
        space_id: SpaceId,
        value: JIRValue,
    ) -> JITResult {
        // Addr index is always zero. When addr is a const, then we just don't access this.
        let addr_index = 0;
        // Value index is 1 if the address is a slice, since it is then the second operand.
        let value_index = if addr.is_slice() { 1 } else { 0 };
        let addr_reg = self.emit_tlb_lookup(inst, addr_index, addr, space_id, value.size(), AccessType::Store)?;
        self.emit_host_store(inst, value_index, addr_reg, value)
    }

    /// Translate the provided guest address into a host address
    /// Returns the register where the host address is stored.
    fn emit_tlb_lookup(
        &mut self,
        inst: InstId,
        addr_index: usize,
        addr: JIRValue,
        space_id: SpaceId,
        size: JIRSize,
        access_type: AccessType,
    ) -> JITResult<HostReg> {
        let temps = self.allocations.get_temp_allocations(inst);
        let tag_reg = temps
            .get_nth_from_class(0, RegClass::Int)
            .expect("We requested this allocation");

        // the tlb lookup assumes that the translated address is in DX
        // TODO: Check page boundaries. Also, how do we handle a cross-page access?
        let addr_reg = match addr {
            JIRValue::Slice(_) => {
                // Address is the second operand.
                reg!(self.allocations, inst, addr_index)
            }
            JIRValue::Const(addr) => {
                let addr_temp = temps
                    .get_nth_from_class(1, RegClass::Int)
                    .expect("We requested this allocation");
                // We load the address into ax, which is specified in clobbers.
                self.load_const(addr_temp, addr)?;
                addr_temp
            }
        }
        .hw_enc();

        let entry_size = TLBEntry::size();
        // Various assertions that are needed for now until this is made more generic.
        // We currently assume addresses of size 8 bytes, and that a tlb entry has
        // a size that is a power of two.
        assert!(entry_size.is_power_of_two());
        assert_eq!(addr.size(), 8);
        let entry_shift: u8 = entry_size
            .trailing_zeros()
            .try_into()
            .expect("The exponent must fit into u8");
        let tmp_addr_reg = self.allocations.scratch_register(RegClass::Int).hw_enc();
        assemble!(self.block.assembler();
            // We copy over the whole address since we need to modify it
            ; mov Rq(tmp_addr_reg), Rq(addr_reg)
        );

        // If the size is 1 the address is always aligned.
        // TODO: We need to check if the size of the data is less than the actual load operation.
        //       If this is the case, then we need to insert an additional check for the larger
        //       access size so that we don't cross page boundaries.
        if size > 1 {
            assert!(size.is_power_of_two());
            assemble!(self.block.assembler();
                // Check alignment
                ; test Rq(addr_reg), (size - 1) as i32
                // TODO: Jump to unaligned access code.
                ; jnz >tlb_miss
            )
        }

        let tlb_offset = X86TaskContext::tlb_offset(access_type);
        let tag_offset = tlb_offset + TLBEntry::tag_offset();
        let target_offset = tlb_offset + TLBEntry::target_offset();
        assemble!(self.block.assembler();
            ; shr Rq(tmp_addr_reg), BYTE EVAddr::<u8>::index_shift_from_start::<PageData>() as _
            ; mov Rq(tag_reg.hw_enc()), Rq(tmp_addr_reg)
            ; and Rd(tmp_addr_reg), DWORD EVAddr::<u8>::index_mask() as _
            ; shr Rq(tag_reg.hw_enc()), BYTE EVAddr::<u8>::tag_shift_from_index() as _
            // TODO can this be removed?
            ; and Rq(tag_reg.hw_enc()), EVAddr::<u8>::tag_mask::<PageData>() as _
            // temp_addr_reg now contains the index.
            // look at tlb_offset + index * entry_size
            // index = index * entry_size
            ; shl Rd(tmp_addr_reg), BYTE entry_shift as _
            // compare tag and entry.tag
            ; gs cmp Rq(tag_reg.hw_enc()), QWORD [Rq(tmp_addr_reg) + tag_offset as _]
            ; jne >tlb_miss
            // TLB hit. Copy the untouched address into the tag_reg which we will reuse here for the address
            ; mov Rq(tag_reg.hw_enc()), Rq(addr_reg)
            ; and Rd(tag_reg.hw_enc()), WORD PageData::page_offset_mask() as _
            // Add target onto the page offset
            ; gs add Rq(tag_reg.hw_enc()), QWORD [Rq(tmp_addr_reg) + target_offset as _]
            // Permissions do not need to be checked, since we have two tlb tables. One for read and one for write.
            // If the address is on a page in the read table, then it is readable, and if it is in the
            // write table, it is writable. Execute permissions are handled via the jit buffer, i.e. code is flushed
            // as soon as it is made non-executable. The permission check is performed on a tlb miss. If the check
            // succeeds, then any future accesses will go through the tlb until the line is flushed.
            ; jmp >success
            ; tlb_miss:
        );

        // TODO: Move the tlb miss code out of the happy path.
        //       We most likely want to defer writing the slow path to the end of the basic block?
        // We emit flush code first, and then the call into the memory helper. On TLB hit this code is skipped.
        // TODO: Does it make sense to only flush those slices that are clobbered by a function call here?
        self.save_loaded_slices()?;

        let Some(callback) = match access_type {
            AccessType::Load => &self.global_context.tlb_miss_read_callback,
            AccessType::Store => &self.global_context.tlb_miss_write_callback,
        }
        .as_ref() else {
            Err(JitError::MissingTlbCallback(access_type))?
        };
        let (translate_trampoline, translate_callback) = (callback.trampoline, callback.callback);
        assemble!(self.block.assembler();
            // call external helper trampoline.
            // trampoline(context, callback, evaddr, size, space_id) -> page_addr
            //    ^^^       ^^^       ^^^     ^^^    ^^^      ^^^          ^^^
            //    eax       rdi       rsi     rdx    rcx      r8           rax
            ; rdgsbase rdi
            ; mov rsi, QWORD translate_callback as _
            // evaddr is already loaded in RDX.
            ; mov rcx, WORD size as _
            ; mov r8d, DWORD space_id.to_index() as _
            // TODO: Call the correct helper function.
            ; mov rax, QWORD translate_trampoline as _
            //// Perform the trampoline call
            // Save the current stack pointer. RBX is preserved by the caller.
            ; mov rbx, rsp
            // Important: Make sure the stack is aligned to 16 bytes according to System V ABI before calling
            ; and rsp, DWORD -16
            // Call the trampoline
            ; call rax
            // restore the stack pointer
            ; mov rsp, rbx
            ; test rax, rax
            // If the translation returns zero, we encountered an error.
            // Jump to the exit code to exit the jit.
            // CAVEAT: The slices are already flushed in preparation for the helper call.
            //         If we change this in the future to only flush clobbered registers,
            //         then we need to flush the rest here.
            ; jz ->exit_failure
            //--- End of trampoline call sequence
            // The result of the helper is the host address of the page, i.e., the target.
            // We reuse the tag_reg here to copy over the target.
            ; mov Rq(tag_reg.hw_enc()), rax
        );
        // Now reload all the flushed slices
        self.reload_loaded_slices()?;
        assemble!(self.block.assembler();
            // Copy the original addres
            ; mov Rq(tmp_addr_reg), Rq(addr_reg)
            // And now we can calculate the page_offset and add it onto the target which is in `tag_reg`
            ; and Rq(tmp_addr_reg), WORD PageData::page_offset_mask() as _
            // Add the page offset onto the target. Target is contained in tag_reg
            ; add Rq(tag_reg.hw_enc()), Rq(tmp_addr_reg)
            // tlb hit skips to here. Regardless of hit or miss, the address will be in addr_gpr if possible.
            ; success:
        );
        Ok(tag_reg)
    }

    /// Emit the code to load the value from memory. The address is assumed to be in RDX
    fn emit_host_load(
        &mut self,
        inst: InstId,
        addr_reg: HostReg,
        output: JIRSlice,
    ) -> JITResult {
        // TODO: Make sure the load does not cross page boundaries.
        let out_reg = reg!(self.allocations, inst, 0);
        let out_reg = out_reg.hw_enc();

        match output.size() {
            1 => {
                assemble!(self.block.assembler();
                    ; movzx Rd(out_reg), BYTE [Rq(addr_reg.hw_enc())]
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; movzx Rd(out_reg), WORD [Rq(addr_reg.hw_enc())]
                );
            }
            4 => {
                assemble!(self.block.assembler();
                    ; mov Rd(out_reg), DWORD [Rq(addr_reg.hw_enc())]
                );
            }
            8 => {
                assemble!(self.block.assembler();
                    ; mov Rq(out_reg), QWORD [Rq(addr_reg.hw_enc())]
                );
            }
            16 => {
                let out_reg_upper = reg!(self.allocations, inst, 1);
                let out_reg_upper = out_reg_upper.hw_enc();

                assemble!(self.block.assembler();
                ; mov Rq(out_reg), QWORD [Rq(addr_reg.hw_enc())]
                ; mov Rq(out_reg_upper), QWORD [Rq(addr_reg.hw_enc()) + 0x8]
                );
            }
            size => todo!("Unsupported load size {size}"),
        }
        Ok(())
    }

    fn emit_host_store(
        &mut self,
        inst: InstId,
        value_index: usize,
        addr_reg: HostReg,
        value: JIRValue,
    ) -> JITResult {
        let size = value.size();
        match size {
            ..=8 => {
                self.emit_host_store_64(inst, value_index, addr_reg, value)?;
            }
            16 => {
                self.emit_host_store_128(inst, value_index, addr_reg, value)?;
            }
            size => todo!("Unsupported size {size}"),
        }

        Ok(())
    }

    // Emit host store instructions for values with size at most 64 bits.
    fn emit_host_store_64(
        &mut self,
        inst: InstId,
        value_index: usize,
        addr_reg: HostReg,
        value: JIRValue,
    ) -> JITResult {
        // TODO: Make sure the store does not cross page boundaries.
        // TODO: This can be optimized to directly move into the memory location.
        //       For JIRConsts this can be done with immediate values.
        let value_reg = match value {
            JIRValue::Slice(_) => {
                reg!(self.allocations, inst, value_index)
            }
            JIRValue::Const(value) => {
                let value_reg = self.allocations.scratch_register(RegClass::Int);
                self.load_const(value_reg, value)?;
                value_reg
            }
        }
        .hw_enc();
        match value.size() {
            1 => {
                assemble!(self.block.assembler();
                    ; mov BYTE [Rq(addr_reg.hw_enc())], Rb(value_reg)
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; mov WORD [Rq(addr_reg.hw_enc())], Rw(value_reg)
                );
            }
            4 => {
                assemble!(self.block.assembler();
                    ; mov DWORD [Rq(addr_reg.hw_enc())], Rd(value_reg)
                );
            }
            8 => {
                assemble!(self.block.assembler();
                    ; mov QWORD [Rq(addr_reg.hw_enc())], Rq(value_reg)
                );
            }
            size => todo!("Unsupported load size {size}"),
        }

        Ok(())
    }

    /// Emit host store instructions for values with size of 128 bit.
    fn emit_host_store_128(
        &mut self,
        inst: InstId,
        value_index: usize,
        addr_reg: HostReg,
        value: JIRValue,
    ) -> JITResult {
        let temp_allocations = self.allocations.get_temp_allocations(inst);
        // TODO: Make sure the store does not cross page boundaries.
        // TODO: This can be optimized to directly move into the memory location.
        //       For JIRConsts this can be done with immediate values.
        let (value_reg_lower, value_reg_upper) = match value {
            JIRValue::Slice(_) => {
                (reg!(self.allocations, inst, value_index), reg!(self.allocations, inst, value_index + 1))
            }
            JIRValue::Const(value) => {
                let (value_lower, value_upper) = value.split(8);

                let value_reg_lower = temp_allocations
                    .get_nth_from_class(1, RegClass::Int)
                    .expect("We requested this");
                self.load_const(value_reg_lower, value_lower)?;
                
                let value_reg_upper = self.allocations.scratch_register(RegClass::Int);
                self.load_const(value_reg_upper, value_upper)?;
                
                (value_reg_lower, value_reg_upper)
            }
        };

        assemble!(self.block.assembler();
        ; mov QWORD [Rq(addr_reg.hw_enc()) + 0x0], Rq(value_reg_lower.hw_enc())
        ; mov QWORD [Rq(addr_reg.hw_enc()) + 0x8], Rq(value_reg_upper.hw_enc())
        );

        Ok(())
    }
}

type TlbMissTrampoline<ContextT> =
    unsafe extern "sysv64" fn(&mut ContextT, *mut (), EVAddr, usize, SpaceId) -> Option<HVAddr<PageData>>;

/// The custom op callback consists of a trampoline function that acts as a transition layer between assembly
/// and rust code. Usually, the rust compiler will be able to inline the closures into this trampoline.
/// The callback signature is defined in [CustomOpImpl]. The callback gets the [JITTaskRuntimeContext] and the [CustomOpArgs].
pub struct TlbMissCallback<ContextT: JITTaskRuntimeContext> {
    trampoline: TlbMissTrampoline<ContextT>,
    callback: *mut (),
    phantom_data: PhantomData<ContextT>,
}

impl<ContextT> TlbMissCallback<ContextT>
where
    ContextT: JITTaskRuntimeContext,
{
    pub fn new<CB>(callback: CB) -> Self
    where
        CB: TlbMissHelper<ContextT>,
    {
        Self {
            trampoline: trampoline::<CB, ContextT>,
            callback: Box::into_raw(Box::new(callback)) as *mut (),
            phantom_data: Default::default(),
        }
    }
}

#[inline]
/// SAFETY: The trampoline is called by the jit runtime. The runtime must ensure that `callback` points to a valid
///         closure.
unsafe extern "sysv64" fn trampoline<CB, ContextT>(
    context: &mut ContextT,
    callback: *mut (),
    addr: EVAddr,
    size: usize,
    space_id: SpaceId,
) -> Option<HVAddr<PageData>>
where
    CB: TlbMissHelper<ContextT>,
    ContextT: JITTaskRuntimeContext,
{
    let callback = callback as *mut CB;
    unsafe { (*callback)(context, addr, size, space_id) }
        .inspect_err(|&e| {
            // Set the error in the runtime context so that it can be retrieved upon exiting
            // the jited code. The JIT will then return the error, instead of continuing execution.
            context.set_runtime_error(e);
        })
        .ok()
}
