use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRSlice,
    MOIRVarNodeId,
};
use tracing::trace;

#[derive(Debug)]
pub struct MOIRInstruction {
    /// The output of the instruction. The output must always be a varnode, since it is written to.
    pub output: MOIRLocalId,
    /// The first input of the instruction.
    pub input_a: Option<MOIRLocal>,
    pub input_b: Option<MOIRLocal>,
    pub operation: MOIROperation,
}

/// This maco provides a convenient wrapper for creating new instructions.
/// The syntax is similar to syntax you would encounter in any programming language.
/// Assignments for example can be created by calling `moir_instruction!(lhs = rhs)`.
/// A binary operation can be created by calling `moir_instruction!(out = lhs op rhs)`
/// A unary operation can be created by calling `moir_instruction!(out = op operand)`
macro_rules! moir_instruction {
    ($lhs:ident = $rhs:ident) => {
        MOIRInstruction::copy($lhs, $rhs)
    };
    ($out:ident = $op:ident $operand:ident) => {
        MOIRInstruction::unary_op($out, $op, $operand)
    };
    ($out:ident = $lhs:ident $op:ident $rhs:ident) => {
        MOIRInstruction::binary_op($out, $lhs, $op, $rhs)
    };
    ($target:ident = *[$space:ident]$addr:ident) => {
        MOIRInstruction::load($target, $addr, $space)
    };
}

use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeUnaryOp,
};
use crate::spec::moir::address_space::MOIRSpaceId;
use crate::spec::moir::scope::MOIRLocalId;
pub(crate) use moir_instruction;

impl MOIRInstruction {
    pub fn load(
        target: MOIRLocalId,
        addr: MOIRLocal,
        space_id: MOIRSpaceId,
    ) -> Self {
        trace!("New instruction: {:?} = *[{:?}]{:?}", target, space_id, addr);
        MOIRInstruction {
            input_a: Some(addr),
            input_b: None,
            output: target,
            operation: MOIROperation::Load(space_id),
        }
    }

    pub fn binary_op(
        out: MOIRLocalId,
        lhs: impl Into<MOIRLocal>,
        op: MOIROperation,
        rhs: impl Into<MOIRLocal>,
    ) -> Self {
        let lhs = lhs.into();
        let rhs = rhs.into();
        trace!("New instruction: {:?} = {:?} {:?} {:?}", out, lhs, op, rhs);
        MOIRInstruction {
            input_a: Some(lhs),
            input_b: Some(rhs),
            output: out,
            operation: op,
        }
    }

    pub fn unary_op(
        out: MOIRLocalId,
        op: MOIROperation,
        operand: impl Into<MOIRLocal>,
    ) -> Self {
        let operand = operand.into();
        trace!("New instruction: {:?} = {:?} {:?}", out, op, operand);
        MOIRInstruction {
            input_a: Some(operand),
            input_b: None,
            output: out,
            operation: op,
        }
    }

    pub fn copy(
        to: MOIRLocalId,
        from: MOIRLocal,
    ) -> Self {
        trace!("New instruction: {:?} = {:?}", to, from);
        MOIRInstruction {
            input_a: Some(from),
            input_b: None,
            output: to,
            operation: MOIROperation::Copy,
        }
    }
}

#[derive(Debug, Clone)]
pub enum MOIROperation {
    Copy,
    CustomOp(Vec<MOIRLocal>),

    // Load and store operations
    Load(MOIRSpaceId),
    Store(MOIRSpaceId),

    // Binary operations
    IntAdd,
    IntSubtract,
    IntMul,
    IntDiv,
    IntSDiv,
    IntRemainder,
    IntSRemainder,
    FloatDiv,
    FloatMul,
    FloatAdd,
    FloatSubtract,
    BoolOr,
    BoolAnd,
    BoolXor,
    IntLeft,
    IntRight,
    IntSRight,
    IntSLess,
    IntSGreater,
    IntSLessEqual,
    IntSGreaterEqual,
    IntLess,
    IntGreater,
    IntLessEqual,
    IntGreaterEqual,
    FloatLess,
    FloatGreater,
    FloatLessEqual,
    FloatGreaterEqual,
    FloatEquals,
    FloatNotEquals,
    IntOr,
    IntAnd,
    IntXor,
    IntEquals,
    IntNotEquals,

    // Unary operations
    BoolNegate,
    BitNegate,
    IntNegate,
    FloatNegate,
}

impl MOIROperation {
    pub fn op_category(&self) -> MOIROpCategory {
        use MOIROperation::*;
        match self {
            BoolOr | BoolAnd | BoolXor | IntSGreater | IntSLessEqual | IntSGreaterEqual | IntLess | IntGreater
            | IntLessEqual | IntGreaterEqual | FloatLess | FloatGreater | FloatLessEqual | FloatGreaterEqual
            | FloatEquals | FloatNotEquals | IntEquals | IntNotEquals => MOIROpCategory::Comparison,
            IntLeft | IntRight | IntSRight => MOIROpCategory::Shift,
            CustomOp(_) => MOIROpCategory::CustomOp,
            _ => MOIROpCategory::Regular,
        }
    }
}

pub enum MOIROpCategory {
    /// Comparison operations always produce outputs with one byte in size
    /// and the inputs are only restricted in that they have to have the
    /// same size, independent of the output.
    Comparison,
    /// Shift operations place no restrictions on the size of the shift amount.
    /// The shiftee and output must however have the same sizes.
    Shift,
    /// Custom operations can have independent input arg sizes and the output is also independent.
    CustomOp,
    /// A regular instruction with no special handling.
    Regular,
}

impl From<SASTPCodeUnaryOp> for MOIROperation {
    fn from(op: SASTPCodeUnaryOp) -> Self {
        match op {
            SASTPCodeUnaryOp::BoolNegate => MOIROperation::BoolNegate,
            SASTPCodeUnaryOp::BitNegate => MOIROperation::BitNegate,
            SASTPCodeUnaryOp::IntNegate => MOIROperation::IntNegate,
            SASTPCodeUnaryOp::FloatNegate => MOIROperation::FloatNegate,
        }
    }
}

impl From<SASTPCodeBinaryOp> for MOIROperation {
    fn from(op: SASTPCodeBinaryOp) -> Self {
        match op {
            SASTPCodeBinaryOp::IntAdd => MOIROperation::IntAdd,
            SASTPCodeBinaryOp::IntSubtract => MOIROperation::IntSubtract,
            SASTPCodeBinaryOp::IntMul => MOIROperation::IntMul,
            SASTPCodeBinaryOp::IntDiv => MOIROperation::IntDiv,
            SASTPCodeBinaryOp::IntSDiv => MOIROperation::IntSDiv,
            SASTPCodeBinaryOp::IntRemainder => MOIROperation::IntRemainder,
            SASTPCodeBinaryOp::IntSRemainder => MOIROperation::IntSRemainder,
            SASTPCodeBinaryOp::FloatDiv => MOIROperation::FloatDiv,
            SASTPCodeBinaryOp::FloatMul => MOIROperation::FloatMul,
            SASTPCodeBinaryOp::FloatAdd => MOIROperation::FloatAdd,
            SASTPCodeBinaryOp::FloatSubtract => MOIROperation::FloatSubtract,
            SASTPCodeBinaryOp::BoolOr => MOIROperation::BoolOr,
            SASTPCodeBinaryOp::BoolAnd => MOIROperation::BoolAnd,
            SASTPCodeBinaryOp::BoolXor => MOIROperation::BoolXor,
            SASTPCodeBinaryOp::IntLeft => MOIROperation::IntLeft,
            SASTPCodeBinaryOp::IntRight => MOIROperation::IntRight,
            SASTPCodeBinaryOp::IntSRight => MOIROperation::IntSRight,
            SASTPCodeBinaryOp::IntSLess => MOIROperation::IntSLess,
            SASTPCodeBinaryOp::IntSGreater => MOIROperation::IntSGreater,
            SASTPCodeBinaryOp::IntSLessEqual => MOIROperation::IntSLessEqual,
            SASTPCodeBinaryOp::IntSGreaterEqual => MOIROperation::IntSGreaterEqual,
            SASTPCodeBinaryOp::IntLess => MOIROperation::IntLess,
            SASTPCodeBinaryOp::IntGreater => MOIROperation::IntGreater,
            SASTPCodeBinaryOp::IntLessEqual => MOIROperation::IntLessEqual,
            SASTPCodeBinaryOp::IntGreaterEqual => MOIROperation::IntGreaterEqual,
            SASTPCodeBinaryOp::FloatLess => MOIROperation::FloatLess,
            SASTPCodeBinaryOp::FloatGreater => MOIROperation::FloatGreater,
            SASTPCodeBinaryOp::FloatLessEqual => MOIROperation::FloatLessEqual,
            SASTPCodeBinaryOp::FloatGreaterEqual => MOIROperation::FloatGreaterEqual,
            SASTPCodeBinaryOp::FloatEquals => MOIROperation::FloatEquals,
            SASTPCodeBinaryOp::FloatNotEquals => MOIROperation::FloatNotEquals,
            SASTPCodeBinaryOp::IntOr => MOIROperation::IntOr,
            SASTPCodeBinaryOp::IntAnd => MOIROperation::IntAnd,
            SASTPCodeBinaryOp::IntXor => MOIROperation::IntXor,
            SASTPCodeBinaryOp::IntEquals => MOIROperation::IntEquals,
            SASTPCodeBinaryOp::IntNotEquals => MOIROperation::IntNotEquals,
        }
    }
}
