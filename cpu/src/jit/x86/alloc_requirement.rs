use crate::jit::host::register::HostRegSet;
use crate::jit::regalloc::requirements::{
    OperandRequest,
    TempRequest,
};
use mogi_sleigh::spec::jir::basic_block::InstId;
use std::ops::Range;

pub struct AllocRequirement<'a> {
    inst: InstId,
    requests_start: usize,
    requests: &'a mut Vec<OperandRequest>,
    instruction_operand_ranges: &'a mut Vec<Range<usize>>,
    clobbers: &'a mut Vec<HostRegSet>,
    required_temps: &'a mut Vec<TempRequest>,
}

impl<'a> AllocRequirement<'a> {
    pub fn new(
        inst: InstId,
        requests: &'a mut Vec<OperandRequest>,
        instruction_operand_ranges: &'a mut Vec<Range<usize>>,
        clobbers: &'a mut Vec<HostRegSet>,
        required_temps: &'a mut Vec<TempRequest>,
    ) -> Self {
        Self {
            inst,
            requests_start: requests.len(),
            requests,
            instruction_operand_ranges,
            clobbers,
            required_temps,
        }
    }

    pub fn add_request(
        &mut self,
        request: OperandRequest,
    ) {
        self.requests.push(request);
    }

    pub fn clobbers_mut(&mut self) -> &mut HostRegSet {
        &mut self.clobbers[self.inst]
    }

    pub fn required_temps_mut(&mut self) -> &mut TempRequest {
        &mut self.required_temps[self.inst]
    }
}

impl Drop for AllocRequirement<'_> {
    fn drop(&mut self) {
        self.instruction_operand_ranges
            .push(self.requests_start..self.requests.len());
    }
}
