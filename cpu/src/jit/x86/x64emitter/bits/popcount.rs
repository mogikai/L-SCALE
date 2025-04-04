use crate::jit::api::JITResult;
use crate::jit::host::register::RegClass;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::regalloc::SliceType;
use crate::jit::x86::x64emitter::slice_load::CopyExtendMode;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRSlice,
    JIRValue,
};

impl X64AllocRequirements {
    pub fn popcount_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));
        match input {
            JIRValue::Slice(jirslice) => {
                requirement.add_request(OperandRequest::int_read_any(jirslice));
            }
            JIRValue::Const(jirconst) => {
                *requirement.required_temps_mut() = TempRequest::new().int(1);
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_popcount(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        let input_reg = match input {
            JIRValue::Slice(input) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                input_reg
            }
            JIRValue::Const(input) => {
                let tempreg = self
                    .allocations
                    .get_temp_allocations(inst)
                    .get_nth_from_class(0, RegClass::Int)
                    .expect("We allocated this above");

                self.load_const(tempreg, input)?;
                tempreg
            }
        };
        match input.size() {
            1 => {
                self.copy_reg(output_reg, input_reg, 1, CopyExtendMode::ZeroExtend(2))?;
                assemble!(self.block.assembler();
                    ; popcnt Rw(output_reg.hw_enc()), Rw(output_reg.hw_enc())
                );
            }
            2 => {
                assemble!(self.block.assembler();
                    ; popcnt Rw(output_reg.hw_enc()), Rw(input_reg.hw_enc())
                );
            }
            4 => {
                assemble!(self.block.assembler();
                    ; popcnt Rd(output_reg.hw_enc()), Rd(input_reg.hw_enc())
                );
            }
            8 => {
                assemble!(self.block.assembler();
                    ; popcnt Rq(output_reg.hw_enc()), Rq(input_reg.hw_enc())
                );
            }
            _ => todo!("Unsupported size"),
        }
        Ok(())
    }
}
