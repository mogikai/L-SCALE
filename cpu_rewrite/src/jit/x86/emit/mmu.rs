use crate::jit::api::{
    JITResult,
    MogiJit,
};
use crate::jit::x86::registers::GPRegister;
use crate::jit::x86::{
    X86MogiJit,
    X86TaskContext,
    assemble,
};
use dynasmrt::DynasmLabelApi;
use mogi_mem::tlb::entry::TLBEntry;
use mogi_mem::tlb::tlb::AccessType;
use mogi_mem::tlb::{
    PageIndex,
    PageTag,
};
use mogi_sleigh::spec::SpaceId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use mogi_utilities::addresses::EVAddr;
use std::mem::offset_of;

impl X86MogiJit {
    pub fn emit_load(
        &mut self,
        output: JIRSlice,
        addr: JIRValue,
        space_id: SpaceId,
    ) -> JITResult {
        // We reserve RDI as temp since it will be used to pass the address to the helper
        // in case of a tlb miss
        let host_address_gpr = self.reserve_gp_as_temp(GPRegister::DI)?;
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
        todo!()
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
        // the tlb lookup assumes that the translated address is in DI
        assert_eq!(translated_address_gpr, GPRegister::DI);
        // TODO: Check page boundaries. Also, how do we handle a cross-page access?
        let addr_gpr = self.get_temp_gpr();
        self.emit_load_value_to_gpr(addr_gpr, addr)?;

        let tag_gpr = self.temp_alloc();
        // TODO: Rewrite temp allocation to automatically free the allocated variables when they go out of scope.
        let tlb_offset = X86TaskContext::tlb_offset(access_type);
        let tag_offset = offset_of!(TLBEntry, tag);
        let target_offset = offset_of!(TLBEntry, target);
        let entry_size = size_of::<TLBEntry>();
        // Various assertions that are needed for now until this is made more generic.
        // We currently assume addresses of size 8 bytes, and that a tlb entry has
        // a size that is a power of two.
        assert!(entry_size.is_power_of_two());
        assert_eq!(addr.size(), 8);
        let entry_shift: u8 = entry_size
            .trailing_zeros()
            .try_into()
            .expect("The exponent must fit into u8");
        assemble!(self.block_mut()?.assembler;
            // We copy over the whole address in case of a tlb miss.
            ; mov Rq(translated_address_gpr as u8), Rq(addr_gpr as u8)
            ; shr Rq(addr_gpr as u8), BYTE EVAddr::<u8>::index_shift_from_start() as _
            ; mov Rq(tag_gpr as u8), Rq(addr_gpr as u8)
            ; and Rd(addr_gpr as u8), DWORD EVAddr::<u8>::index_mask() as _
            ; shr Rq(tag_gpr as u8), BYTE EVAddr::<u8>::tag_shift_from_index() as _
            ; and Rq(tag_gpr as u8), EVAddr::<u8>::tag_mask() as _
            // addr_gpr now contains the index.
            // look at tlb_offset + index * entry_size
            // index = index * entry_size
            ; shl Rd(addr_gpr as u8), BYTE entry_shift as _
            // compare tag and entry.tag
            ; cmp Rq(tag_gpr as u8), QWORD [Rq(addr_gpr as u8) + tlb_offset as _ + cpu_context]
            ; jne >tlb_miss
            // TLB hit. Extract the page offset from the EVAddr
            ; and Rd(translated_address_gpr as u8), WORD 0xfff
            // Add target onto the page offset
            ; add Rq(translated_address_gpr as u8), QWORD [Rq(addr_gpr as u8) + target_offset as _ + cpu_context]
            ; jmp >success
        );

        // We emit flush code first, and then the call into the memory helper. On TLB hit this code is skipped.
        // TODO: Only flush registers that are not preserved by callee
        self.preserve_registers_caller()?;

        assemble!(self.block_mut()?.assembler;
            ; tlb_miss:
            // call external helper. The helper should have a signature along the lines of
            // helper(evaddr, size) -> page_addr
            // evaddr is already loaded in RDI.
            ; mov rsi, WORD size as _
            ; mov edx, DWORD usize::from(space_id) as _
            // TODO: Call the correct helper function.
            ; mov rax, QWORD 0x1234
            ; call rax
            // The result of the helper is the host address
            ; mov Rq(translated_address_gpr as u8), rax
            // tlb hit skips to here. Regardless of hit or miss, the address will be in addr_gpr if possible.
            ; success:
        );
        self.temp_free(tag_gpr)
    }

    fn emit_host_load(
        &mut self,
        output: JIRSlice,
        host_address_gpr: GPRegister,
    ) -> JITResult {
        // TODO: Make sure the load does not cross page boundaries.
        // Allocate the output slice gpr, or get an existing one.
        let out_gpr = self.get_slice_gp(output);
        match output.size() {
            1 => {
                assemble!(self.block_mut()?.assembler;
                    ; movzx Rd(out_gpr as u8), BYTE [Rq(host_address_gpr as u8)]
                );
            }
            2 => {
                assemble!(self.block_mut()?.assembler;
                    ; movzx Rd(out_gpr as u8), WORD [Rq(host_address_gpr as u8)]
                );
            }
            3..=4 => {
                assemble!(self.block_mut()?.assembler;
                    ; mov Rd(out_gpr as u8), DWORD [Rq(host_address_gpr as u8)]
                );
            }
            5..=8 => {
                assemble!(self.block_mut()?.assembler;
                    ; mov Rq(out_gpr as u8), QWORD [Rq(host_address_gpr as u8)]
                );
            }
            _ => {
                todo!()
            }
        }
        Ok(())
    }
}
