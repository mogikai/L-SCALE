use crate::jit::api::JITResult;
use crate::jit::host::register::RegClass;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use crate::jit::x86::x64emitter::helpers::reg;
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
        input.is_slice_then(|s| requirement.add_request(OperandRequest::int_read_any(*s)));
    }
}

impl X64Emitter<'_> {
    pub fn emit_popcount(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let output_reg = reg!(self.allocations, inst, 0);

        let input_reg = match input {
            JIRValue::Slice(input) => reg!(self.allocations, inst, 1),
            JIRValue::Const(input) => {
                let tempreg = self.allocations.scratch_register(RegClass::Int);
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
