use crate::jit::api::JITResult;
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
        self.emit_tlb_lookup(addr, space_id, AccessType::Load)
    }

    pub fn emit_store(
        &mut self,
        addr: JIRValue,
        space_id: SpaceId,
        value: JIRValue,
    ) -> JITResult {
        todo!()
    }

    fn emit_tlb_lookup(
        &mut self,
        addr: JIRValue,
        space_id: SpaceId,
        access_type: AccessType,
    ) -> JITResult {
        let addr_gpr = match addr {
            JIRValue::Slice(addr) => self.load_slice_gpr(addr),
            JIRValue::Const(addr) => {
                let addr_gpr = self.get_temp_gpr();
                self.emit_load_const_to_gpr(addr_gpr, addr)?;
                addr_gpr
            }
        };

        let tag_gpr = self.temp_alloc();
        // TODO: Rewrite temp allocation to automatically free the allocated variables when they go out of scope.
        let tlb_offset = X86TaskContext::tlb_offset(access_type);
        let tag_offset = offset_of!(TLBEntry, tag);
        let target_offset = offset_of!(TLBEntry, target);
        let entry_size = size_of::<TLBEntry>();
        assert!(entry_size.is_power_of_two());
        let entry_shift = entry_size.trailing_zeros();
        assemble!(self.block_mut()?.assembler;
            ; shr Rq(addr_gpr as u8), EVAddr::<u8>::index_shift_from_start() as _
            ; mov Rq(tag_gpr as u8), Rq(addr_gpr as u8)
            ; and Rq(addr_gpr as u8), EVAddr::<u8>::index_mask() as _
            ; shr Rq(tag_gpr as u8), EVAddr::<u8>::tag_shift_from_index() as _
            ; and Rq(tag_gpr as u8), EVAddr::<u8>::tag_mask() as _
            // addr_gpr now contains the index.
            // look at tlb_offset + index * entry_size
            // index = index * entry_size
            ; shl Rq(addr_gpr as u8), entry_shift as _
            // compare tag and entry.tag
            ; cmp Rq(tag_gpr as u8), QWORD [Rq(addr_gpr as u8) + tlb_offset as _ + cpu_context]
            ; jne >tlb_miss
            // TLB hit. Load target into addr_gpr
            ; mov Rq(addr_gpr as u8), QWORD [Rq(addr_gpr as u8) + target_offset as _ + cpu_context]
            ; jmp >success
        );

        assemble!(self.block_mut()?.assembler;
            ; tlb_miss:
            // call external helper
            ; mov Rq(addr_gpr as u8), rax
            ; success:
        );
        self.temp_free(tag_gpr)
    }
}
