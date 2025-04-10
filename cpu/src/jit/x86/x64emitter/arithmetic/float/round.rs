use crate::jit::host::register::RegClass;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst, JIRSlice, JIRValue
};

use crate::jit::api::JITResult;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};

enum RoundingMode {
    Ceil, // round towards positive infinity
    Floor, // round towards negative infinity
    Round, // round towards the closest integral value
}

impl X64AllocRequirements {
    pub fn round_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::vec_write_any(output));

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
    pub fn emit_ceil(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Ceil)
    }

    pub fn emit_floor(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Floor)
    }

    pub fn emit_round(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Round)
    }

    fn emit_round_with_mode(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        rounding_mode: RoundingMode
    ) -> JITResult {
        let in_size = input.size();
        let out_size = output.size();
        assert_eq!(in_size, out_size, "Sizes of slices must be equal when rounding");

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

        let rounding_mode_imm = match rounding_mode {
            RoundingMode::Ceil => 0b10,
            RoundingMode::Floor => 0b01,
            RoundingMode::Round => 0b00,
        };

        match in_size {
            4 => {
                // single-precision float rounding
                assemble!(self.block.assembler();
                    ; roundss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), rounding_mode_imm
                );
            }
            8 => {
                // double-precision float to QWORD integer
                assemble!(self.block.assembler();
                    ; roundsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), rounding_mode_imm
                );
            }
            size => todo!("Unsupported size {:?}", size),
        }

        Ok(())
    }
}
