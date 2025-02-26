use crate::jit::api::{
    JITResult,
    JITTaskRuntimeContext,
    JitError,
    TlbMissHelper,
};
use crate::jit::x86::regalloc::SliceType;
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::task_context::X86TaskContext;
use crate::jit::x86::{
    X86MogiJit,
    assemble,
};
use crate::tlb::entry::TLBEntry;
use crate::tlb::{
    AccessType,
    PageIndex,
    PageMetaInfo,
    PageTag,
};
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_utilities::addresses::{
    EVAddr,
    HVAddr,
};
use std::marker::PhantomData;

impl<PageDataT> X86MogiJit<PageDataT>
where
    PageDataT: PageMetaInfo,
{
    pub fn emit_load(
        &mut self,
        output: JIRSlice,
        addr: JIRValue,
        space_id: SpaceId,
    ) -> JITResult {
        // We reserve RDX as temp since it will be used to pass the address to the helper
        // in case of a tlb miss
        let host_address_gpr = self.reserve_gp_as_temp(GPRegister::DX)?;
        self.emit_tlb_lookup(host_address_gpr, addr, space_id, output.size(), AccessType::Load)?;
        self.emit_host_load(output, host_address_gpr)?;
        self.temp_free(host_address_gpr)
    }

    pub fn emit_store(
        &mut self,
        addr: JIRValue,
        space_id: SpaceId,
        value: JIRValue,
    ) -> JITResult {
        let host_address_gpr = self.reserve_gp_as_temp(GPRegister::DX)?;
        self.emit_tlb_lookup(host_address_gpr, addr, space_id, value.size(), AccessType::Store)?;
        self.emit_host_store(value, host_address_gpr)?;
        self.temp_free(host_address_gpr)
    }

    /// Translate the provided guest address into a host address
    fn emit_tlb_lookup(
        &mut self,
        translated_address_gpr: GPRegister,
        addr: JIRValue,
        space_id: SpaceId,
        size: JIRSize,
        access_type: AccessType,
    ) -> JITResult {
        // the tlb lookup assumes that the translated address is in DX
        assert_eq!(translated_address_gpr, GPRegister::DX);
        // TODO: Check page boundaries. Also, how do we handle a cross-page access?
        let addr_gpr = self.get_temp_gpr();
        self.emit_load_value_to_gpr(addr_gpr, addr)?;

        let tag_gpr = self.temp_alloc();
        // TODO: Rewrite temp allocation to automatically free the allocated variables when they go out of scope.
        let tlb_offset = X86TaskContext::<PageDataT>::tlb_offset(access_type);
        let tag_offset = tlb_offset + TLBEntry::<PageDataT>::tag_offset();
        let target_offset = tlb_offset + TLBEntry::<PageDataT>::target_offset();
        let entry_size = TLBEntry::<PageDataT>::size();
        // Various assertions that are needed for now until this is made more generic.
        // We currently assume addresses of size 8 bytes, and that a tlb entry has
        // a size that is a power of two.
        assert!(entry_size.is_power_of_two());
        assert_eq!(addr.size(), 8);
        let entry_shift: u8 = entry_size
            .trailing_zeros()
            .try_into()
            .expect("The exponent must fit into u8");
        assemble!(self.block_mut()?.assembler();
            // We copy over the whole address in case of a tlb miss.
            ; mov Rq(translated_address_gpr as u8), Rq(addr_gpr as u8)
        );

        // If the size is 1 the address is always aligned.
        // TODO: We need to check if the size of the data is less than the actual load operation.
        //       If this is the case, then we need to insert an additional check for the larger
        //       access size so that we don't cross page boundaries.
        if size > 1 {
            assert!(size.is_power_of_two());
            assemble!(self.block_mut()?.assembler();
                // Check alignment
                ; test Rq(addr_gpr as u8), (size - 1) as i32
                // TODO: Jump to unaligned access code.
                ; jnz >tlb_miss
            )
        }

        assemble!(self.block_mut()?.assembler();
            ; shr Rq(addr_gpr as u8), BYTE EVAddr::<u8>::index_shift_from_start::<PageDataT>() as _
            ; mov Rq(tag_gpr as u8), Rq(addr_gpr as u8)
            ; and Rd(addr_gpr as u8), DWORD EVAddr::<u8>::index_mask() as _
            ; shr Rq(tag_gpr as u8), BYTE EVAddr::<u8>::tag_shift_from_index() as _
            ; and Rq(tag_gpr as u8), EVAddr::<u8>::tag_mask::<PageDataT>() as _
            // addr_gpr now contains the index.
            // look at tlb_offset + index * entry_size
            // index = index * entry_size
            ; shl Rd(addr_gpr as u8), BYTE entry_shift as _
            // compare tag and entry.tag
            ; cmp Rq(tag_gpr as u8), QWORD [Rq(addr_gpr as u8) + tag_offset as _ + cpu_context]
            ; jne >tlb_miss
            // TLB hit. Extract the page offset from the EVAddr
            ; and Rd(translated_address_gpr as u8), WORD PageDataT::page_offset_mask() as _
            // Add target onto the page offset
            ; add Rq(translated_address_gpr as u8), QWORD [Rq(addr_gpr as u8) + target_offset as _ + cpu_context]
            // Permissions do not need to be checked, since we have two tlb tables. One for read and one for write.
            // If the address is on a page that is in the read table, then it is readable, and if it is in the
            // write table, it is writable. Execute permissions are handled via the jit buffer, i.e. code is flushed
            // as soon as it is made non-executable. The permission check is performed on a tlb miss. If the check
            // succeeds, then any future accesses will go through the tlb until the line is flushed.
            ; jmp >success
        );

        // TODO: Move the tlb miss code out of the happy path.
        //       We most likely want to defer writing the slow path to the end of the basic block?
        // We emit flush code first, and then the call into the memory helper. On TLB hit this code is skipped.
        self.preserve_registers_caller()?;

        let Some(callback) = match access_type {
            AccessType::Load => &self.global_context.tlb_miss_read_callback,
            AccessType::Store => &self.global_context.tlb_miss_write_callback,
        }
        .as_ref() else {
            Err(JitError::MissingTlbCallback(access_type))?
        };
        let (translate_trampoline, translate_callback) = (callback.trampoline, callback.callback);
        assemble!(self.block_mut()?.assembler();
            ; tlb_miss:
            // call external helper trampoline.
            // trampoline(context, callback, evaddr, size, space_id) -> page_addr
            //    ^^^       ^^^       ^^^     ^^^    ^^^      ^^^          ^^^
            //    eax       rdi       rsi     rdx    rcx      r8           rax
            ; mov rdi, cpu_context
            ; mov rsi, QWORD translate_callback as _
            // evaddr is already loaded in RDX.
            ; mov rcx, WORD size as _
            ; mov r8d, DWORD usize::from(space_id) as _
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
            ; jz ->exit_failure
            //--- End of trampoline call sequence
            // The result of the helper is the host address
            ; mov Rq(translated_address_gpr as u8), rax
            // tlb hit skips to here. Regardless of hit or miss, the address will be in addr_gpr if possible.
            ; success:
        );
        // We reserve the translated_address_gpr again here, since the flush cleared the reservation.
        // This does not touch the contents of the register, since it is marked as free.
        self.reserve_gp_as_temp(translated_address_gpr)?;
        self.temp_free(tag_gpr)
    }

    fn emit_host_load(
        &mut self,
        output: JIRSlice,
        host_address_gpr: GPRegister,
    ) -> JITResult {
        // TODO: Make sure the load does not cross page boundaries.
        // Allocate the output slice gpr, or get an existing one.
        let out_gpr = self.allocate_slice_gpr(output, SliceType::Output);
        match output.size() {
            1 => {
                assemble!(self.block_mut()?.assembler();
                    ; movzx Rd(out_gpr as u8), BYTE [Rq(host_address_gpr as u8)]
                );
            }
            2 => {
                assemble!(self.block_mut()?.assembler();
                    ; movzx Rd(out_gpr as u8), WORD [Rq(host_address_gpr as u8)]
                );
            }
            3..=4 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov Rd(out_gpr as u8), DWORD [Rq(host_address_gpr as u8)]
                );
            }
            5..=8 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov Rq(out_gpr as u8), QWORD [Rq(host_address_gpr as u8)]
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
        value: JIRValue,
        host_address_gpr: GPRegister,
    ) -> JITResult {
        // TODO: Make sure the store does not cross page boundaries.
        // TODO: This can be optimized to directly move into the memory location.
        //       For JIRConsts this can be done with immediate values.
        let out_gpr = match value {
            JIRValue::Slice(slice) => self.load_slice_gpr(slice, SliceType::Input),
            JIRValue::Const(value) => {
                let out_gpr = self.temp_alloc();
                self.emit_load_const_to_gpr(out_gpr, value)?;
                out_gpr
            }
        };
        match value.size() {
            1 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov BYTE [Rq(host_address_gpr as u8)], Rb(out_gpr as u8)
                );
            }
            2 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov WORD [Rq(host_address_gpr as u8)], Rw(out_gpr as u8)
                );
            }
            3..=4 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov DWORD [Rq(host_address_gpr as u8)], Rd(out_gpr as u8)
                );
            }
            5..=8 => {
                assemble!(self.block_mut()?.assembler();
                    ; mov QWORD [Rq(host_address_gpr as u8)], Rq(out_gpr as u8)
                );
            }
            _ => {
                todo!()
            }
        }
        if let JIRValue::Const(_) = value {
            self.temp_free(out_gpr)?;
        }
        Ok(())
    }
}

type TlbMissTrampoline<ContextT, PageDataT> =
    unsafe extern "sysv64" fn(&mut ContextT, *mut (), EVAddr, usize, SpaceId) -> Option<HVAddr<PageDataT>>;

/// The custom op callback consists of a trampoline function that acts as a transition layer between assembly
/// and rust code. Usually, the rust compiler will be able to inline the closures into this trampoline.
/// The callback signature is defined in [CustomOpImpl]. The callback gets the [JITTaskRuntimeContext] and the [CustomOpArgs].
pub struct TlbMissCallback<ContextT: JITTaskRuntimeContext> {
    trampoline: TlbMissTrampoline<ContextT, ContextT::PageData>,
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
) -> Option<HVAddr<ContextT::PageData>>
where
    CB: TlbMissHelper<ContextT>,
    ContextT: JITTaskRuntimeContext,
{
    let callback = callback as *mut CB;
    unsafe { (*callback)(context, addr, size, space_id) }
        .map_err(|e| {
            // Set the error in the runtime context so that it can be retrieved upon exiting
            // the jited code. The JIT will then return the error, instead of continuing execution.
            context.set_runtime_error(e);
            e
        })
        .ok()
}
