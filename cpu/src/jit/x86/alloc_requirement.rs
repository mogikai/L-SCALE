use crate::jit::host::register::HostRegSet;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use std::ops::Range;

use super::x64emitter::X64InstructionAllocRequirement;

pub struct AllocRequirement<'a> {
    inst: InstId,
    requests_start: usize,
    requests: &'a mut Vec<OperandRequest>,
    alloc_requirements: &'a mut Vec<X64InstructionAllocRequirement>,
}

impl<'a> AllocRequirement<'a> {
    pub fn new(
        inst: InstId,
        requests: &'a mut Vec<OperandRequest>,
        alloc_requirements: &'a mut Vec<X64InstructionAllocRequirement>,
    ) -> Self {
        alloc_requirements.push(Default::default());

        Self {
            inst,
            requests_start: requests.len(),
            requests,
            alloc_requirements,
        }
    }

    pub fn add_request(
        &mut self,
        request: OperandRequest,
    ) {
        self.requests.push(request);
    }

    pub fn clobbers_mut(&mut self) -> &mut HostRegSet {
        &mut self.alloc_requirements[self.inst].clobbers
    }

    pub fn required_temps_mut(&mut self) -> &mut TempRequest {
        &mut self.alloc_requirements[self.inst].required_temps
    }

    pub fn set_clear_temps(&mut self) {
        self.alloc_requirements[self.inst].clear_temporary_allocations = true;
    }
}

impl Drop for AllocRequirement<'_> {
    fn drop(&mut self) {
        self.alloc_requirements[self.inst].operand_range = self.requests_start..self.requests.len();
    }
}
