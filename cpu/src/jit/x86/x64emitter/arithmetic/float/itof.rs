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

impl X64AllocRequirements {
    pub fn itof_requirements(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        input: JIRValue,
    ) {
        requirement.add_request(OperandRequest::vec_write_any(output));

        match input {
            JIRValue::Slice(jirslice) => {
                requirement.add_request(OperandRequest::int_read_any(jirslice));
            }
            JIRValue::Const(jirconst) => {
                // no additional registers necessary
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that convert `input_a` to a float and store the result in `output`.
    #[inline]
    pub fn emit_itof(
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

        match input {
            JIRValue::Const(input) => {
                // const => directly emit result
                let result_jirconst = match (in_size, out_size) {
                    (4, 4) => {
                        // DWORD integer to single-precision
                        let input = input.value() as i32;
                        JIRConst::new((input as f32).to_bits().into(), 4)
                    }
                    (8, 4) => {
                        // QWORD integer single-precision
                        let input = input.value() as i64;
                        JIRConst::new((input as f32).to_bits().into(), 4)
                    }
                    (4, 8) => {
                        // DWORD integer to double-precision
                        let input = input.value() as i32;
                        JIRConst::new((input as f64).to_bits().into(), 8)
                    }
                    (8, 8) => {
                        // QWORD integer double-precision
                        let input = input.value() as i64;
                        JIRConst::new((input as f64).to_bits().into(), 8)
                    }
                    size => todo!("Unsupported size {:?}", size),
                };

                self.load_const(output_reg, result_jirconst)?;
            }
            JIRValue::Slice(_) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                match (in_size, out_size) {
                    (4, 4) => {
                        // DWORD integer to single-precision float
                        assemble!(self.block.assembler();
                            ; vcvtsi2ss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rd(input_reg.hw_enc())
                        );
                    }
                    (4, 8) => {
                        // DWORD integer to double-precision float
                        assemble!(self.block.assembler();
                            ; vcvtsi2sd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rd(input_reg.hw_enc())
                        );
                    }
                    (8, 4) => {
                        // QWORD integer to single-precision float
                        assemble!(self.block.assembler();
                            ; vcvtsi2ss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rq(input_reg.hw_enc())
                        );
                    }
                    (8, 8) => {
                        // QWORD integer to double-precision float
                        assemble!(self.block.assembler();
                            ; vcvtsi2sd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rq(input_reg.hw_enc())
                        );
                    }
                    size => todo!("Unsupported size {:?}", size),
                }
            }
        };

        Ok(())
    }
}
