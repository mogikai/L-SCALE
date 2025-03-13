use mogi_sleigh::spec::jir::values::JIRSlice;

#[derive(Default, Debug)]
pub struct Register;

#[derive(Debug)]
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
    FixedReg(Register),
}

#[derive(Debug)]
pub enum OperandKind {
    /// The operand is written to
    Write,
    /// The operand is only read from.
    Read,
}

#[derive(Debug)]
pub enum OperandRequest {
    /// A temp operand is requested. The allocator will reserve a register for this temporary request that can be used
    /// for the instruction. There are no guarantees to the data contained in the register, and the register is free
    /// to be reused after the instruction.
    Temp,
    /// Request a slice to be allocated. The [OperandConstraint] specifies whether to always allocate a register,
    /// or even to request a specific register. The [OperandKind] specifies whether the operand is written to, or
    /// read from.
    Slice {
        slice: JIRSlice,
        constraint: OperandConstraint,
        kind: OperandKind,
    },
}

impl OperandRequest {
    pub fn slice_write_any(slice: JIRSlice) -> Self {
        OperandRequest::Slice {
            slice,
            constraint: OperandConstraint::Any,
            kind: OperandKind::Write,
        }
    }

    pub fn slice_read_any(slice: JIRSlice) -> Self {
        OperandRequest::Slice {
            slice,
            constraint: OperandConstraint::Any,
            kind: OperandKind::Read,
        }
    }

    pub fn temp() -> Self {
        OperandRequest::Temp
    }
}

// TODO: Optimize sizes and layout later.
#[derive(Default, Debug)]
pub struct RegisterSet {
    registers: Vec<Register>,
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
    ) -> &RegisterSet;

    /// Return the required operands for the provided instruction.
    fn operands(
        &self,
        inst_id: usize,
    ) -> &[OperandRequest];
}
