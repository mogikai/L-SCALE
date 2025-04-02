use crate::jit::host::register::{
    HostReg,
    HostRegSet,
    RegClass,
};
use mogi_sleigh::spec::jir::values::JIRSlice;

#[derive(Debug, Copy, Clone)]
pub enum OperandConstraint {
    /// The operand may be loaded into a register, or used directly from memory.
    /// This effectively means that the allocator is free to not allocate a register
    /// to the operand, such that the instruction must directly use the backing memory.
    /// In case of free registers however, the allocator will assign a register.
    /// This gives the allocator the freedom to not spill registers for arguments that are
    /// for example only used once, where a spill and load would incur overhead vs just
    /// using a memory reference as argument to the actual instruction.
    Any,
    /// The operand must be loaded into any register.
    Reg,
    /// The operand must be put into the provided register.
    FixedReg(HostReg),
}

#[derive(Debug, Copy, Clone)]
pub enum OperandKind {
    /// The operand is written to
    Write,
    /// The operand is only read from.
    Read,
}

#[derive(Debug, Copy, Clone)]
pub enum OperandPos {
    Before,
    After,
}

// TODO: We might need to compress the requests into an easily copyable value by doing
//       some bit manipulation?
/// Request a slice to be allocated. The [OperandConstraint] specifies whether to always allocate a register,
/// or even to request a specific register. The [OperandKind] specifies whether the operand is written to, or
/// read from.
#[derive(Debug)]
pub struct OperandRequest {
    pub slice: JIRSlice,
    pub constraint: OperandConstraint,
    pub kind: OperandKind,
    pub class: RegClass,
}

impl OperandRequest {
    pub fn int_write_any(slice: JIRSlice) -> Self {
        OperandRequest {
            slice,
            constraint: OperandConstraint::Any,
            kind: OperandKind::Write,
            class: RegClass::Int,
        }
    }

    pub fn int_read_any(slice: JIRSlice) -> Self {
        OperandRequest {
            slice,
            constraint: OperandConstraint::Any,
            kind: OperandKind::Read,
            class: RegClass::Int,
        }
    }
}

/// This struct provides some convenience view methods on the operand request slice.
#[derive(Debug)]
pub struct OperandRequirements<'a>(&'a [OperandRequest]);

impl<'a> OperandRequirements<'a> {
    pub fn new(requests: &'a [OperandRequest]) -> OperandRequirements<'a> {
        OperandRequirements(requests)
    }

    /// Returns an iterator over all operand requests with a fixed register.
    pub fn fixed(&self) -> impl Iterator<Item = &OperandRequest> {
        self.0
            .iter()
            .filter(|req| matches!(req.constraint, OperandConstraint::FixedReg(_)))
    }

    pub fn write(&self) -> impl Iterator<Item = (usize, &OperandRequest)> {
        self.all()
            .filter(|(_, req)| matches!(req.kind, OperandKind::Write))
    }

    pub fn read(&self) -> impl Iterator<Item = (usize, &OperandRequest)> {
        self.all()
            .filter(|(_, req)| matches!(req.kind, OperandKind::Read))
    }

    pub fn all(&self) -> impl Iterator<Item = (usize, &OperandRequest)> + ExactSizeIterator {
        self.0.iter().enumerate()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// This trait specifies methods needed by the allocator to determine the required registers for an instruction.
/// The emitter implementation will need to specify what operands and temporaries are needed per instruction.
pub trait EmitterAllocRequirements {
    /// The clobber registers needed for the instruction. The allocator will make sure that there is no
    /// data in these registers that needs to be persisted, and it will make sure that none of the data
    /// is reused later on. Clobbered registers may be allocated again after the instruction is emitted.
    fn clobbers(
        &self,
        inst_id: usize,
    ) -> HostRegSet;

    /// Return the required operands for the provided instruction.
    fn operands(
        &self,
        inst_id: usize,
    ) -> OperandRequirements;

    /// Returns the total number of different operands used in this emitter instance.
    fn total_opereands(&self) -> usize;
}
