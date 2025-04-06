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
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use crate::tlb::entry::TLBEntry;
use crate::tlb::{
    AccessType,
    PageData,
    PageIndex,
    PageMetaInfo,
    PageTag,
};
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
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
        requirement.add_request(OperandRequest::int_write_any(output));
        // The address might be in a slice, so request it if it is not const.
        match addr {
            JIRValue::Slice(addr) => {
                requirement.add_request(OperandRequest::int_read_reg(addr));
                *requirement.required_temps_mut() = TempRequest::new().int(1);
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg())
            }
            JIRValue::Const(_) => {
                *requirement.required_temps_mut() = TempRequest::new().int(2);
                *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg());
            }
        }
    }

    pub fn store_requirements(
        requirement: &mut AllocRequirement,
        addr: JIRValue,
        _space_id: SpaceId,
        value: JIRValue,
    ) {
        *requirement.required_temps_mut() = TempRequest::new().int(2);
        addr.is_slice_then(|addr| {
            *requirement.required_temps_mut() = TempRequest::new().int(1);
            requirement.add_request(OperandRequest::int_read_any(*addr))
        });
        value.is_slice_then(|value| requirement.add_request(OperandRequest::int_read_any(*value)));
        *requirement.clobbers_mut() = HostRegSet::empty().with(GPRegister::DX.into_host_reg())
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
            let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
                todo!()
            };
            let JIRValue::Const(addr) = addr else {
                panic!("Load from const space must have a constant address")
            };
            // Const address is interpreted as the value at that address.
            // TODO: Perform these const loads in the optimizer instead and propagate the constants.
            return self.load_const(output_reg, addr);
        }
        // Address is the second operand
        let addr_index = 1;
        self.emit_tlb_lookup(inst, addr_index, addr, space_id, output.size(), AccessType::Load)?;
        self.emit_host_load(inst, output)
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
        self.emit_tlb_lookup(inst, addr_index, addr, space_id, value.size(), AccessType::Store)?;
        self.emit_host_store(inst, value_index, value)
    }

    /// Translate the provided guest address into a host address
    fn emit_tlb_lookup(
        &mut self,
        inst: InstId,
        addr_index: usize,
        addr: JIRValue,
        space_id: SpaceId,
        size: JIRSize,
        access_type: AccessType,
    ) -> JITResult {
        let temps = self.allocations.get_temp_allocations(inst);
        let tag_reg = temps
            .get_nth_from_class(0, RegClass::Int)
            .expect("We requested this allocation")
            .hw_enc();

        // the tlb lookup assumes that the translated address is in DX
        // TODO: Check page boundaries. Also, how do we handle a cross-page access?
        let addr_gpr = match addr {
            JIRValue::Slice(_) => {
                // Address is the second operand.
                let Allocation::Register(reg) = self.allocations.get_allocation(inst, addr_index) else {
                    unreachable!("We requested a register allocation")
                };
                reg
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
        assemble!(self.block.assembler();
            // We copy over the whole address in case of a tlb miss.
            ; mov rdx, Rq(addr_gpr)
        );

        // If the size is 1 the address is always aligned.
        // TODO: We need to check if the size of the data is less than the actual load operation.
        //       If this is the case, then we need to insert an additional check for the larger
        //       access size so that we don't cross page boundaries.
        if size > 1 {
            assert!(size.is_power_of_two());
            assemble!(self.block.assembler();
                // Check alignment
                ; test Rq(addr_gpr), (size - 1) as i32
                // TODO: Jump to unaligned access code.
                ; jnz >tlb_miss
            )
        }

        let tlb_offset = X86TaskContext::tlb_offset(access_type);
        let tag_offset = tlb_offset + TLBEntry::tag_offset();
        let target_offset = tlb_offset + TLBEntry::target_offset();
        assemble!(self.block.assembler();
            ; shr Rq(addr_gpr), BYTE EVAddr::<u8>::index_shift_from_start::<PageData>() as _
            ; mov Rq(tag_reg), Rq(addr_gpr)
            ; and Rd(addr_gpr), DWORD EVAddr::<u8>::index_mask() as _
            ; shr Rq(tag_reg), BYTE EVAddr::<u8>::tag_shift_from_index() as _
            ; and Rq(tag_reg), EVAddr::<u8>::tag_mask::<PageData>() as _
            // addr_gpr now contains the index.
            // look at tlb_offset + index * entry_size
            // index = index * entry_size
            ; shl Rd(addr_gpr), BYTE entry_shift as _
            // compare tag and entry.tag
            ; cmp Rq(tag_reg), QWORD [Rq(addr_gpr) + tag_offset as _ + cpu_context]
            ; jne >tlb_miss
            // TLB hit. Extract the page offset from the EVAddr
            ; and edx, WORD PageData::page_offset_mask() as _
            // Add target onto the page offset
            ; add rdx, QWORD [Rq(addr_gpr) + target_offset as _ + cpu_context]
            // Permissions do not need to be checked, since we have two tlb tables. One for read and one for write.
            // If the address is on a page in the read table, then it is readable, and if it is in the
            // write table, it is writable. Execute permissions are handled via the jit buffer, i.e. code is flushed
            // as soon as it is made non-executable. The permission check is performed on a tlb miss. If the check
            // succeeds, then any future accesses will go through the tlb until the line is flushed.
            ; jmp >success
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
            ; tlb_miss:
            // call external helper trampoline.
            // trampoline(context, callback, evaddr, size, space_id) -> page_addr
            //    ^^^       ^^^       ^^^     ^^^    ^^^      ^^^          ^^^
            //    eax       rdi       rsi     rdx    rcx      r8           rax
            ; mov rdi, cpu_context
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
            // The result of the helper is the host address
            ; mov rdx, rax
            // tlb hit skips to here. Regardless of hit or miss, the address will be in addr_gpr if possible.
            ; success:
        );
        self.reload_loaded_slices()?;
        Ok(())
    }

    /// Emit the code to load the value from memory. The address is assumed to be in RDX
    fn emit_host_load(
        &mut self,
        inst: InstId,
        output: JIRSlice,
    ) -> JITResult {
        // TODO: Make sure the load does not cross page boundaries.
        let Allocation::Register(out_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };
        let out_reg = out_reg.hw_enc();
        match output.size() {
            1 => {
                assemble!(self.block.assembler();
                    ; movzx Rd(out_reg), BYTE [rdx]
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; movzx Rd(out_reg), WORD [rdx]
                );
            }
            3..=4 => {
                assemble!(self.block.assembler();
                    ; mov Rd(out_reg), DWORD [rdx]
                );
            }
            5..=8 => {
                assemble!(self.block.assembler();
                    ; mov Rq(out_reg), QWORD [rdx]
                );
            }
            _ => {
                todo!()
            }
        }
        Ok(())
    }

    fn emit_host_store(
        &mut self,
        inst: InstId,
        value_index: usize,
        value: JIRValue,
    ) -> JITResult {
        let temp_allocations = self.allocations.get_temp_allocations(inst);
        // TODO: Make sure the store does not cross page boundaries.
        // TODO: This can be optimized to directly move into the memory location.
        //       For JIRConsts this can be done with immediate values.
        let value_reg = match value {
            JIRValue::Slice(_) => {
                let Allocation::Register(value_reg) = self.allocations.get_allocation(inst, value_index) else {
                    todo!()
                };
                value_reg
            }
            JIRValue::Const(value) => {
                let value_reg = temp_allocations
                    .get_nth_from_class(0, RegClass::Int)
                    .expect("We requested this");
                self.load_const(value_reg, value)?;
                value_reg
            }
        }
        .hw_enc();
        match value.size() {
            1 => {
                assemble!(self.block.assembler();
                    ; mov BYTE [rdx], Rb(value_reg as u8)
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; mov WORD [rdx], Rw(value_reg as u8)
                );
            }
            3..=4 => {
                assemble!(self.block.assembler();
                    ; mov DWORD [rdx], Rd(value_reg as u8)
                );
            }
            5..=8 => {
                assemble!(self.block.assembler();
                    ; mov QWORD [rdx], Rq(value_reg as u8)
                );
            }
            _ => {
                todo!()
            }
        }
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
    black_box({
        let callback = callback as *mut CB;
        unsafe { (*callback)(context, addr, size, space_id) }
            .map_err(|e| {
                // Set the error in the runtime context so that it can be retrieved upon exiting
                // the jited code. The JIT will then return the error, instead of continuing execution.
                context.set_runtime_error(e);
                e
            })
            .ok()
    })
}
