use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRSlice,
    VarNodeId,
};
use std::fmt::{
    write,
    Display,
    Formatter,
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

impl Display for MOIRInstruction {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self.operation {
            MOIROperation::ZeroExtend | MOIROperation::SignExtend => {
                write!(
                    f,
                    "{} = {}({})",
                    self.output,
                    self.operation,
                    self.input_a.expect("Unary op has to have input_a")
                )
            }
            MOIROperation::Load(space) => {
                write!(f, "{} = *[{:?}]{}", self.output, space, self.input_a.expect("Load has to have input_a"))
            }
            MOIROperation::Copy => {
                write!(f, "{} = {}", self.output, self.input_a.expect("Copy has to have input_a"))
            }
            MOIROperation::CustomOp(_) => {
                write!(f, "{} = customOp", self.output)
            }
            MOIROperation::AddressOf
            | MOIROperation::BoolNegate
            | MOIROperation::BitNegate
            | MOIROperation::IntNegate
            | MOIROperation::FloatNegate => {
                write!(f, "{} = {}{}", self.output, self.operation, self.input_a.expect("Unary op has to have input_a"))
            }
            _ => write!(
                f,
                "{} = {} {} {}",
                self.output,
                self.input_a.expect("Binary op must have input_a"),
                self.operation,
                self.input_b.expect("Binary op must have input_b")
            ),
        }
    }
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
    ($lhs:ident = &$rhs:ident) => {
        MOIRInstruction::address_of($lhs, $rhs)
    };
    ($lhs:ident = zext($rhs:ident)) => {
        MOIRInstruction::zext($lhs, $rhs)
    };
    ($target:ident = *[$space:ident]$addr:ident) => {
        MOIRInstruction::load($target, $addr, $space)
    };
}

use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeUnaryOp,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::scope::MOIRLocalId;
pub(crate) use moir_instruction;

impl MOIRInstruction {
    pub fn load(
        target: MOIRLocalId,
        addr: MOIRLocal,
        space_id: SpaceId,
    ) -> Self {
        trace!("New instruction: {:?} = *[{:?}]{:?}", target, space_id, addr);
        MOIRInstruction {
            input_a: Some(addr),
            input_b: None,
            output: target,
            operation: MOIROperation::Load(space_id),
        }
    }

    pub fn address_of(
        lhs: MOIRLocalId,
        rhs: MOIRLocal,
    ) -> Self {
        trace!("New instruction: {:?} = &{:?}", lhs, rhs);
        MOIRInstruction {
            output: lhs,
            input_a: Some(rhs),
            input_b: None,
            operation: MOIROperation::AddressOf,
        }
    }

    pub fn zext(
        lhs: MOIRLocalId,
        rhs: MOIRLocal,
    ) -> Self {
        trace!("New instruction: {:?} = zext({:?})", lhs, rhs);
        MOIRInstruction {
            output: lhs,
            input_a: Some(rhs),
            input_b: None,
            operation: MOIROperation::ZeroExtend,
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

    // Special operations
    /// Special AddressOf operation. This is translated into a constant, when the constructor is called.
    AddressOf,
    /// Zero extends the input if its size is less than the output.
    /// If the sizes are equal this operation is optimized away.
    /// If the input size is larger than the output size, this
    /// will result in a size error.
    ZeroExtend,
    /// Sign extends the input if its size is less than the output.
    /// If the sizes are equal this operation is optimized away.
    /// If the input size is larger than the output size, this
    /// will result in a size error.
    SignExtend,

    // Load and store operations
    // When these operations are resolved into JIR upon calling the constructor, all values
    // that refer to a `const` space are translated into actual constants, where their offset
    // is interpreted as the constants' value.
    Load(SpaceId),
    Store(SpaceId),

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

impl Display for MOIROperation {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIROperation::Copy => write!(f, ""),
            MOIROperation::CustomOp(args) => write!(f, "custom"),
            MOIROperation::AddressOf => write!(f, "&"),
            MOIROperation::ZeroExtend => write!(f, "zext"),
            MOIROperation::SignExtend => write!(f, "sext"),
            MOIROperation::Load(_) => write!(f, "load"),
            MOIROperation::Store(_) => write!(f, "store"),
            MOIROperation::IntAdd => write!(f, "+"),
            MOIROperation::IntSubtract => write!(f, "-"),
            MOIROperation::IntMul => write!(f, "*"),
            MOIROperation::IntDiv => write!(f, "/"),
            MOIROperation::IntSDiv => write!(f, "s/"),
            MOIROperation::IntRemainder => write!(f, "%"),
            MOIROperation::IntSRemainder => write!(f, "s%"),
            MOIROperation::FloatDiv => write!(f, "f/"),
            MOIROperation::FloatMul => write!(f, "f*"),
            MOIROperation::FloatAdd => write!(f, "f+"),
            MOIROperation::FloatSubtract => write!(f, "f-"),
            MOIROperation::BoolOr => write!(f, "||"),
            MOIROperation::BoolAnd => write!(f, "&&"),
            MOIROperation::BoolXor => write!(f, "^^"),
            MOIROperation::IntLeft => write!(f, "<<"),
            MOIROperation::IntRight => write!(f, ">>"),
            MOIROperation::IntSRight => write!(f, "s>>"),
            MOIROperation::IntSLess => write!(f, "s<"),
            MOIROperation::IntSGreater => write!(f, "s>"),
            MOIROperation::IntSLessEqual => write!(f, "s<="),
            MOIROperation::IntSGreaterEqual => write!(f, "s>="),
            MOIROperation::IntLess => write!(f, "<"),
            MOIROperation::IntGreater => write!(f, ">"),
            MOIROperation::IntLessEqual => write!(f, "<="),
            MOIROperation::IntGreaterEqual => write!(f, ">="),
            MOIROperation::FloatLess => write!(f, "f<"),
            MOIROperation::FloatGreater => write!(f, "f>"),
            MOIROperation::FloatLessEqual => write!(f, "f<="),
            MOIROperation::FloatGreaterEqual => write!(f, "f>="),
            MOIROperation::FloatEquals => write!(f, "f=="),
            MOIROperation::FloatNotEquals => write!(f, "f!="),
            MOIROperation::IntOr => write!(f, "|"),
            MOIROperation::IntAnd => write!(f, "&"),
            MOIROperation::IntXor => write!(f, "^"),
            MOIROperation::IntEquals => write!(f, "=="),
            MOIROperation::IntNotEquals => write!(f, "!="),
            MOIROperation::BoolNegate => write!(f, "!"),
            MOIROperation::BitNegate => write!(f, "~"),
            MOIROperation::IntNegate => write!(f, "-"),
            MOIROperation::FloatNegate => write!(f, "f-"),
        }
    }
}

impl MOIROperation {
    pub fn op_category(&self) -> MOIROpCategory {
        use MOIROperation::*;
        match self {
            BoolOr | BoolAnd | BoolXor | IntSGreater | IntSLess | IntSLessEqual | IntSGreaterEqual | IntLess
            | IntGreater | IntLessEqual | IntGreaterEqual | FloatLess | FloatGreater | FloatLessEqual
            | FloatGreaterEqual | FloatEquals | FloatNotEquals | IntEquals | IntNotEquals => MOIROpCategory::Comparison,
            IntLeft | IntRight | IntSRight => MOIROpCategory::Shift,
            CustomOp(_) | ZeroExtend | Load(_) | Store(_) => MOIROpCategory::Special,
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
    /// This category also includes special pcode operations like zext.
    /// The size dependance of input and output ultimately must be decided by looking at the individual
    /// operation.
    Special,
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
