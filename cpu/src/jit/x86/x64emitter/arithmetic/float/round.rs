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
    JIRConst,
    JIRSlice,
    JIRValue,
};

use crate::jit::api::JITResult;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};

enum RoundingMode {
    Ceil,  // round towards positive infinity
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
                // no additional registers required
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_ceil(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Ceil)
    }

    pub fn emit_floor(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Floor)
    }

    pub fn emit_round(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        self.emit_round_with_mode(inst, output, input, RoundingMode::Round)
    }

    fn emit_round_with_mode(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
        rounding_mode: RoundingMode,
    ) -> JITResult {
        let in_size = input.size();
        let out_size = output.size();
        assert_eq!(in_size, out_size, "Sizes of slices must be equal when rounding");

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        // either the slice register or a temporary register which contains the value
        match input {
            JIRValue::Const(jirconst) => {
                // const value => directly load result
                let result = rounding_mode.round(jirconst);
                self.load_const(output_reg, result)?;
            }
            JIRValue::Slice(jirslice) => {
                // slice value => emit instructions
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
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
                            ; vroundss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc()), rounding_mode_imm
                        );
                    }
                    8 => {
                        // double-precision float to QWORD integer
                        assemble!(self.block.assembler();
                            ; vroundsd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc()), rounding_mode_imm
                        );
                    }
                    size => todo!("Unsupported size {:?}", size),
                }
            }
        }

        Ok(())
    }
}

impl RoundingMode {
    fn round(&self, input: JIRConst) -> JIRConst {
        let size = input.size();

        match size {
            4 => {
                // single-precision rounding
                let value = f32::from_bits(u32::try_from(input.value()).expect("This must fit"));
                match self {
                    RoundingMode::Ceil => JIRConst::new(value.ceil().to_bits().into(), 4),
                    RoundingMode::Floor => JIRConst::new(value.floor().to_bits().into(), 4),
                    RoundingMode::Round => JIRConst::new(value.round().to_bits().into(), 4),
                }
            }
            8 => {
                // double-precision rounding
                let value = f64::from_bits(u64::try_from(input.value()).expect("This must fit"));
                match self {
                    RoundingMode::Ceil => JIRConst::new(value.ceil().to_bits().into(), 8),
                    RoundingMode::Floor => JIRConst::new(value.floor().to_bits().into(), 8),
                    RoundingMode::Round => JIRConst::new(value.round().to_bits().into(), 8),
                }
            }
            size => todo!("Unsupported size {size}")
        }
    }
}
