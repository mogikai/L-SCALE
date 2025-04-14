use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst, JIRSlice, JIRValue
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
                // no additional registers required
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that convert a float to an integer by truncating towards zero.
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

        match input {
            JIRValue::Const(input) => {
                // directly write result
                let result_const = match in_size {
                    4 => {
                        // single-precision truncate
                        JIRConst::new(f32::from_bits(input.value().try_into().expect("This must fit")).trunc().to_bits().into(), out_size)
                    }
                    8 => {
                        // double-precision truncate
                        JIRConst::new(f64::from_bits(input.value().try_into().expect("This must fit")).trunc().to_bits().into(), out_size)
                    }
                    size => todo!("Unsupported input size {in_size}")
                };

                self.load_const(output_reg, result_const)?;

            }
            JIRValue::Slice(input) => {
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!()
                };

                match (in_size, out_size) {
                    (4, 4) => {
                        // single-precision truncate to dword singed integer
                        assemble!(self.block.assembler();
                            ; vcvttss2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                        )
                    }
                    (4, 8) => {
                        // single-precision truncate to qword singed integer
                        assemble!(self.block.assembler();
                            ; vcvttss2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                        )
                    }
                    (8, 4) => {
                        // double-precision truncate to dword singed integer
                        assemble!(self.block.assembler();
                            ; vcvttsd2si Rd(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                        )
                    }
                    (8, 8) => {
                        // double-precision truncate to qword singed integer
                        assemble!(self.block.assembler();
                            ; vcvttsd2si Rq(output_reg.hw_enc()), Rx(input_reg.hw_enc())
                        )
                    }
                    size => todo!("Unsupported size {:?}", size),
                }
            }
        };

        Ok(())
    }
}
