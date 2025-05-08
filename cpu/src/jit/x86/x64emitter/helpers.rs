use crate::jit::host::register::{
    HostReg,
    RegClass,
};
use crate::jit::regalloc::requirements::{
    OperandConstraint,
    OperandKind,
    OperandRequest,
};
use crate::jit::x86::alloc_requirement::AllocRequirement;
use crate::jit::x86::x64emitter::X64AllocRequirements;
use mogi_sleigh::spec::jir::values::JIRSlice;

impl X64AllocRequirements {
    /// Request two allocations with the provided input if it is larger than 8 bytes.
    /// Otherwise, only request one allocation.
    pub fn require_large_int_read_any(
        requirement: &mut AllocRequirement,
        input: JIRSlice,
    ) {
        match input.size() {
            size if size <= 8 => {
                requirement.add_request(OperandRequest::int_read_any(input));
            }
            size if size == 16 => {
                let (lower, upper) = input.split(8);
                requirement.add_request(OperandRequest::int_read_any(lower));
                requirement.add_request(OperandRequest::int_read_any(upper));
            }
            _ => panic!("Unsupported slice size"),
        }
    }

    /// Request two allocations with the provided input if it is larger than 8 bytes.
    /// Otherwise, only request one allocation.
    pub fn require_large_int_write_any(
        requirement: &mut AllocRequirement,
        input: JIRSlice,
    ) {
        match input.size() {
            size if size <= 8 => {
                requirement.add_request(OperandRequest::int_write_any(input));
            }
            size if size == 16 => {
                let (lower, upper) = input.split(8);
                requirement.add_request(OperandRequest::int_write_any(lower));
                requirement.add_request(OperandRequest::int_write_any(upper));
            }
            _ => panic!("Unsupported slice size"),
        }
    }

    /// Request two allocations with the provided input if it is larger than 8 bytes.
    /// Otherwise, only request one allocation.
    pub fn require_large_int_write_fixed(
        requirement: &mut AllocRequirement,
        output: JIRSlice,
        fixed_lower: HostReg,
        fixed_upper: HostReg,
    ) {
        match output.size() {
            size if size <= 8 => {
                requirement.add_request(OperandRequest::int_write_fixed(fixed_lower, output));
            }
            size if size == 16 => {
                let (lower, upper) = output.split(8);
                requirement.add_request(OperandRequest::int_write_fixed(fixed_lower, lower));
                requirement.add_request(OperandRequest::int_write_fixed(fixed_upper, upper));
            }
            _ => panic!("Unsupported slice size"),
        }
    }
}

macro_rules! reg {
    ($allocations:expr, $inst:expr, $op_id:expr) => {
        match $allocations.get_allocation($inst, $op_id) {
            crate::jit::regalloc::result::Allocation::Register(reg) => reg,
            _ => panic!("We expected a register allocation."),
        }
    };
}
pub(crate) use reg;
