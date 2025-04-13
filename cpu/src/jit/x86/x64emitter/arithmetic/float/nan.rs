use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::assemble;
use crate::jit::x86::x64emitter::copy::CopyExtendMode;
use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::common::JIRSize;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use tracing::trace;

use crate::jit::api::JITResult;
use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::result::Allocation;
use crate::jit::x86::x64emitter::{
    X64AllocRequirements,
    X64Emitter,
};


impl X64AllocRequirements {
    pub fn nan_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::int_write_any(output));

        match input {
            JIRValue::Slice(input) => {
                requirement.add_request(OperandRequest::vec_read_any(input));
            }
            JIRValue::Const(_) => {
                // no additional requirements, directly write result
            }
        }
    }
}

impl X64Emitter<'_> {
    pub fn emit_nan(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let size = input.size();

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!();
        };

        // a float can be checked for NaN by comparing it against itself and checking for the
        // unordered result, i.e. ZF=1,PF=1,CF=1

        match input {
            JIRValue::Slice(input) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!();
                };

                match size {
                    4 => {
                        // single-precision NaN check
                        assemble!(self.block.assembler();
                            ; ucomiss Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc())
                            ; setp Rb(output_reg.hw_enc())
                        );
                    }
                    8 => {
                        // double-precision NaN check
                        assemble!(self.block.assembler();
                            ; ucomisd Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc())
                            ; setp Rb(output_reg.hw_enc())
                        );
                    }
                    size => todo!("Unsupported size {size}")
                }
            }
            JIRValue::Const(input) => {
                let result = match size {
                    4 => {
                        // single-precision const NaN check
                        let input = f32::from_bits(input.value().try_into().expect("Input must be a valid float"));
                        input.is_nan() as u8
                    }
                    8 => {
                        // double-precision const NaN check
                        let input = f64::from_bits(input.value().try_into().expect("Input must be a valid float"));
                        input.is_nan() as u8
                    }
                    size => todo!("Unsupported size {size}")
                };

                self.load_const(output_reg, JIRConst::new(result.into(), 1).into())?;
            }
        }

        Ok(())
    }
}
