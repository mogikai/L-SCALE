use mogi_sleigh::spec::jir::basic_block::InstId;
use mogi_sleigh::spec::jir::values::{
    JIRConst,
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
    pub fn ftof_requirements(
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
                // no registers necessary, directly write result
            }
        }
    }
}

impl X64Emitter<'_> {
    /// Emit instructions that converts floating-point to floating-point with
    /// different precision.
    #[inline]
    pub fn emit_ftof(
        &mut self,
        inst: InstId,
        output: JIRSlice,
        input: JIRValue,
    ) -> JITResult {
        let in_size = input.size();
        let out_size = output.size();
        assert_ne!(in_size, out_size, "Floating-point conversion must change precision");

        let Allocation::Register(output_reg) = self.allocations.get_allocation(inst, 0) else {
            todo!()
        };

        match input {
            JIRValue::Const(input) => {
                // const input => directly load result
                let output_jirconst = match (in_size, out_size) {
                    (4, 8) => {
                        // single-precision to double-precision
                        let input: f32 = f32::from_bits(input.value().try_into().expect("This must fit"));
                        let output: f64 = input as f64;
                        JIRConst::new(output.to_bits().into(), 8)
                    }
                    (8, 4) => {
                        // double-precision to single-precision
                        let input: f64 = f64::from_bits(input.value().try_into().expect("This must fit"));
                        let output: f32 = input as f32;
                        JIRConst::new(output.to_bits().into(), 4)
                    }
                    size => todo!("Unsupported size {:?}", size),
                };

                self.load_const(output_reg, output_jirconst)?;
            }
            JIRValue::Slice(input) => {
                // slice => emit conversion instruction
                let Allocation::Register(input_reg) = self.allocations.get_allocation(inst, 1) else {
                    todo!();
                };

                match (in_size, out_size) {
                    (4, 8) => {
                        // single-precision to double-precision
                        assemble!(self.block.assembler();
                            ; vcvtss2sd Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc())
                        );
                    }
                    (8, 4) => {
                        // double-precision to single-precision
                        assemble!(self.block.assembler();
                            ; vcvtsd2ss Rx(output_reg.hw_enc()), Rx(input_reg.hw_enc()), Rx(input_reg.hw_enc())
                        );
                    }
                    size => todo!("Unsupported size {:?}", size),
                }
            }
        };

        Ok(())
    }
}
