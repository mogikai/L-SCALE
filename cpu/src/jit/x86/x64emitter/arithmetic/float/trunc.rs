use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

use crate::jit::api::JITResult;
use crate::jit::host::register::RegClass;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};

impl X64AllocRequirements {
    pub fn trunc_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));

        match input {
            JIRValue::Slice(jirslice) => {
                requirement.add_request(OperandRequest::vec_read_any(jirslice));
            }
            JIRValue::Const(jirconst) => {
                *requirement.required_temps_mut() = TempRequest::new().vector(1);
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_trunc(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let in_size = input.size();
        let out_size = output.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        // either the slice register or a temporary register which contains the value
        let input_reg = match input {
            JIRValue::Const(jirconst) => {
                // load input into register if not already in one
                let input_reg = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Vector)
                    .expect("We allocated this above");
                self.load_const(input_reg, jirconst)?;
                input_reg
            }
            JIRValue::Slice(jirslice) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                input_reg
            }
        };

        match (in_size, out_size) {
            (4, 4) => {
                // single-precision truncate to dword singed integer
                assemble!(self.block.assembler();
                    ; cvttss2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                )
            }
            (4, 8) => {
                // single-precision truncate to qword singed integer
                assemble!(self.block.assembler();
                    ; cvttss2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                )
            }
            (8, 4) => {
                // double-precision truncate to dword singed integer
                assemble!(self.block.assembler();
                    ; cvttsd2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                )
            }
            (8, 8) => {
                // double-precision truncate to qword singed integer
                assemble!(self.block.assembler();
                    ; cvttsd2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                )
            }
            size => todo!("Unsupported size {:?}", size),
        }

        Ok(())
    }
}
