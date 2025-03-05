use crate::spec::moir::varnode::MOIRLocal;
use mogi_sleigh_parser::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeUnaryOp,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::fmt::{
    Display,
    Formatter,
};
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MOIRInstruction {
    /// The output of the instruction. The output must always be a varnode, since it is written to.
    /// Some operations may have no output at all.
    pub output: Option<MOIRLocalId>,
    /// The first input of the instruction.
    pub input_a: Option<MOIRLocal>,
    pub input_b: Option<MOIRLocal>,
    pub operation: MOIROperation,
    pub label: Option<LabelId>,
}

impl Display for MOIRInstruction {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        if let Some(label) = self.label {
            writeln!(f, "<{label}>")?;
        }
        match &self.operation {
            MOIROperation::Build(table) => write!(f, "build {table}"),
            MOIROperation::PopCount
            | MOIROperation::LZCount
            | MOIROperation::ZeroExtend
            | MOIROperation::SignExtend
            | MOIROperation::Nan
            | MOIROperation::Abs
            | MOIROperation::Sqrt
            | MOIROperation::IntToFloat
            | MOIROperation::FloatToFloat
            | MOIROperation::Trunc
            | MOIROperation::Ceil
            | MOIROperation::Floor
            | MOIROperation::Round => {
                write!(
                    f,
                    "{} = {}({})",
                    self.output.expect("Extend ops have an output"),
                    self.operation,
                    self.input_a.expect("Extend ops have to have input_a")
                )
            }
            MOIROperation::Carry | MOIROperation::SignCarry | MOIROperation::SignBorrow => {
                write!(
                    f,
                    "{} = {}({}, {})",
                    self.output.expect("op has to have an output"),
                    self.operation,
                    self.input_a.expect("op has to have input_a"),
                    self.input_b.expect("op has to have input_a")
                )
            }
            MOIROperation::Load(space) => {
                write!(
                    f,
                    "{} = *[{:?}]{}",
                    self.output.expect("Load ops have an output"),
                    space,
                    self.input_a.expect("Load has to have input_a")
                )
            }
            MOIROperation::Store(space) => {
                write!(
                    f,
                    "*[{:?}]{} = {}",
                    space,
                    self.input_a.expect("Store has to have input_a"),
                    self.input_b.expect("Store has to have input_b")
                )
            }
            MOIROperation::Copy => {
                write!(
                    f,
                    "{} = {}",
                    self.output.expect("Copy ops have an output"),
                    self.input_a.expect("Copy has to have input_a")
                )
            }
            MOIROperation::CustomOp { op_id, extra_args } => {
                if let Some(output) = self.output {
                    write!(f, "{} = {}(", output, op_id)?
                } else {
                    write!(f, "{}(", op_id)?
                }
                let mut inputs = vec![];
                if let Some(input) = self.input_a {
                    inputs.push(input);
                }
                if let Some(input) = self.input_b {
                    inputs.push(input);
                }
                inputs.extend(extra_args);
                let len = inputs.len();
                if len >= 2 {
                    for input in inputs[..(len - 1)].iter() {
                        write!(f, "{}, ", input)?;
                    }
                }
                if len >= 1 {
                    write!(f, "{})", inputs.last().expect("There has to be at least one element"))
                } else {
                    write!(f, ")")
                }
            }
            MOIROperation::AddressOf
            | MOIROperation::BoolNegate
            | MOIROperation::BitNegate
            | MOIROperation::IntNegate
            | MOIROperation::FloatNegate => {
                write!(
                    f,
                    "{} = {}{}",
                    self.output.expect("Unary op has to have an output"),
                    self.operation,
                    self.input_a.expect("Unary op has to have input_a")
                )
            }
            MOIROperation::GotoDirect(ty) => write!(f, "{ty} [{}]", self.input_a.expect("Goto has to have input_a")),
            MOIROperation::GotoIndirect(ty) => write!(f, "{ty} {}", self.input_a.expect("Goto has to have input_a")),
            MOIROperation::GotoLabel(ty, label) => write!(f, "{ty} <{:?}>", label),
            MOIROperation::GotoDirectIf => write!(
                f,
                "if ({}) goto [{}]",
                self.input_b.expect("GotoIf has to have input_b"),
                self.input_a.expect("Goto has to have input_a")
            ),
            MOIROperation::GotoIndirectIf => write!(
                f,
                "if ({}) goto {}",
                self.input_b.expect("GotoIf has to have input_b"),
                self.input_a.expect("Goto has to have input_a")
            ),
            MOIROperation::GotoLabelIf(label) => {
                write!(f, "if ({}) goto <{:?}>", self.input_b.expect("GotoIf has to have input_b"), label)
            }
            MOIROperation::Nop => write!(f, "nop"),
            _ => write!(
                f,
                "{} = {} {} {}",
                self.output.expect("Binary op has to have an output"),
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
    (nop) => {
        MOIRInstruction::nop()
    };
    (build $table:tt) => {
        MOIRInstruction::build($table)
    };
    ($target:ident = *[$space:ident]$addr:ident) => {
        MOIRInstruction::load($target, $addr, $space)
    };
    (*[$space:ident]$addr:ident = $value:ident) => {
        MOIRInstruction::store($addr, $space, $value)
    };
    (if ($cond:tt) goto [$target:tt]) => {
        MOIRInstruction::goto_indirect_if($target, $cond)
    };
    (if ($cond:tt) goto $target:tt) => {
        MOIRInstruction::goto_direct_if($target, $cond)
    };
    (if ($cond:tt) goto <$target:tt>) => {
        MOIRInstruction::goto_label_if($target, $cond)
    };
    (goto [$target:tt]) => {
        MOIRInstruction::goto_indirect($target, crate::spec::moir::code::MOIRGotoType::Regular)
    };
    (goto $target:tt) => {
        MOIRInstruction::goto_direct($target, crate::spec::moir::code::MOIRGotoType::Regular)
    };
    (goto <$target:tt>) => {
        MOIRInstruction::goto_label($target, crate::spec::moir::code::MOIRGotoType::Regular)
    };
    (call [$target:tt]) => {
        MOIRInstruction::goto_indirect($target, crate::spec::moir::code::MOIRGotoType::Call)
    };
    (call $target:tt) => {
        MOIRInstruction::goto_direct($target, crate::spec::moir::code::MOIRGotoType::Call)
    };
    (call <$target:tt>) => {
        MOIRInstruction::goto_label($target, crate::spec::moir::code::MOIRGotoType::Call)
    };
    (return [$target:tt]) => {
        MOIRInstruction::goto_indirect($target, crate::spec::moir::code::MOIRGotoType::Return)
    };
    (return $target:tt) => {
        MOIRInstruction::goto_direct($target, crate::spec::moir::code::MOIRGotoType::Return)
    };
    (return <$target:tt>) => {
        MOIRInstruction::goto_label($target, crate::spec::moir::code::MOIRGotoType::Return)
    };
    ($out:ident = $lhs:tt - $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntSubtract, $rhs)
    };
    ($out:ident = $lhs:tt ^ $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntXor, $rhs)
    };
    ($out:ident = $lhs:tt & $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntAnd, $rhs)
    };
    ($out:ident = $lhs:tt | $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntOr, $rhs)
    };
    ($out:ident = $lhs:tt >> $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntRight, $rhs)
    };
    ($out:ident = $lhs:tt << $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::IntLeft, $rhs)
    };
    ($out:ident = $lhs:tt $op:ident $rhs:tt) => {
        MOIRInstruction::binary_op($out, $lhs, $op, $rhs)
    };
    ($lhs:ident = &$rhs:ident) => {
        MOIRInstruction::address_of($lhs, $rhs)
    };
    ($lhs:ident = popcount($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::PopCount, $rhs)
    };
    ($lhs:ident = lzcount($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::LZCount, $rhs)
    };
    ($lhs:ident = zext($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::ZeroExtend, $rhs)
    };
    ($lhs:ident = sext($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::SignExtend, $rhs)
    };
    ($out:ident = carry($lhs:tt, $rhs:tt)) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::Carry, $rhs)
    };
    ($out:ident = scarry($lhs:tt, $rhs:tt)) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::SignCarry, $rhs)
    };
    ($out:ident = sborrow($lhs:tt, $rhs:tt)) => {
        MOIRInstruction::binary_op($out, $lhs, crate::spec::moir::code::MOIROperation::SignBorrow, $rhs)
    };
    ($lhs:ident = nan($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Nan, $rhs)
    };
    ($lhs:ident = abs($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Abs, $rhs)
    };
    ($lhs:ident = sqrt($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Sqrt, $rhs)
    };
    ($lhs:ident = int2float($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::IntToFloat, $rhs)
    };
    ($lhs:ident = float2float($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::FloatToFloat, $rhs)
    };
    ($lhs:ident = trunc($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Trunc, $rhs)
    };
    ($lhs:ident = ceil($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Ceil, $rhs)
    };
    ($lhs:ident = floor($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Floor, $rhs)
    };
    ($lhs:ident = round($rhs:tt)) => {
        MOIRInstruction::unary_op($lhs, crate::spec::moir::code::MOIROperation::Round, $rhs)
    };
    ($output:ident = $op_id:tt($args:tt)) => {
        MOIRInstruction::custom_op($output, $op_id, $args)
    };
    ($lhs:ident = $rhs:tt) => {
        MOIRInstruction::copy($lhs, $rhs)
    };
    ($out:ident = ~$operand:tt) => {
        MOIRInstruction::unary_op($out, crate::spec::moir::code::MOIROperation::BitNegate, $operand)
    };
    ($out:ident = $op:tt $operand:tt) => {
        MOIRInstruction::unary_op($out, $op, $operand)
    };
}

use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::code::MOIROperation::{
    GotoDirectIf,
    GotoIndirectIf,
};
use crate::spec::moir::custom_op::CustomOpId;
use crate::spec::moir::scope::{
    LabelId,
    MOIRLocalId,
};
use crate::spec::moir::table::TableId;
pub(crate) use moir_instruction;

impl MOIRInstruction {
    pub fn nop() -> Self {
        MOIRInstruction {
            output: None,
            input_a: None,
            input_b: None,
            operation: MOIROperation::Nop,
            label: None,
        }
    }

    pub fn build(table: TableId) -> Self {
        MOIRInstruction {
            output: None,
            input_a: None,
            input_b: None,
            operation: MOIROperation::Build(table),
            label: None,
        }
    }

    pub fn custom_op<T, V>(
        output: Option<MOIRLocalId>,
        op_id: CustomOpId,
        args: V,
    ) -> Self
    where
        V: Deref<Target = [T]>,
        T: Into<MOIRLocal> + Copy,
    {
        MOIRInstruction {
            output,
            input_a: args.first().copied().map(|arg| arg.into()),
            input_b: args.get(1).copied().map(|arg| arg.into()),
            operation: MOIROperation::CustomOp {
                op_id,
                extra_args: args
                    .split_at_checked(2)
                    .map(|(_, args)| args.iter().copied().map(|arg| arg.into()).collect())
                    .unwrap_or_default(),
            },
            label: None,
        }
    }

    pub fn load(
        target: MOIRLocalId,
        addr: impl Into<MOIRLocal>,
        space_id: SpaceId,
    ) -> Self {
        MOIRInstruction {
            output: Some(target),
            input_a: Some(addr.into()),
            input_b: None,
            operation: MOIROperation::Load(space_id),
            label: None,
        }
    }

    pub fn store(
        addr: impl Into<MOIRLocal>,
        space_id: SpaceId,
        value: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: Some(addr.into()),
            input_b: Some(value.into()),
            operation: MOIROperation::Store(space_id),
            label: None,
        }
    }

    pub fn address_of(
        lhs: MOIRLocalId,
        rhs: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: Some(lhs),
            input_a: Some(rhs.into()),
            input_b: None,
            operation: MOIROperation::AddressOf,
            label: None,
        }
    }

    pub fn binary_op(
        out: MOIRLocalId,
        lhs: impl Into<MOIRLocal>,
        op: MOIROperation,
        rhs: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: Some(out),
            input_a: Some(lhs.into()),
            input_b: Some(rhs.into()),
            operation: op,
            label: None,
        }
    }

    pub fn goto_direct_if(
        target: impl Into<MOIRLocal>,
        cond: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: Some(target.into()),
            input_b: Some(cond.into()),
            operation: MOIROperation::GotoDirectIf,
            label: None,
        }
    }

    pub fn goto_indirect_if(
        target: impl Into<MOIRLocal>,
        cond: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: Some(target.into()),
            input_b: Some(cond.into()),
            operation: MOIROperation::GotoIndirectIf,
            label: None,
        }
    }

    pub fn goto_label_if(
        target: LabelId,
        cond: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: None,
            input_b: Some(cond.into()),
            operation: MOIROperation::GotoLabelIf(target),
            label: None,
        }
    }

    pub fn goto_direct(
        target: impl Into<MOIRLocal>,
        marker: MOIRGotoType,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: Some(target.into()),
            input_b: None,
            operation: MOIROperation::GotoDirect(marker),
            label: None,
        }
    }

    pub fn goto_indirect(
        target: impl Into<MOIRLocal>,
        marker: MOIRGotoType,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: Some(target.into()),
            input_b: None,
            operation: MOIROperation::GotoIndirect(marker),
            label: None,
        }
    }

    pub fn goto_label(
        target: LabelId,
        marker: MOIRGotoType,
    ) -> Self {
        MOIRInstruction {
            output: None,
            input_a: None,
            input_b: None,
            operation: MOIROperation::GotoLabel(marker, target),
            label: None,
        }
    }

    pub fn unary_op(
        out: MOIRLocalId,
        op: MOIROperation,
        operand: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: Some(out),
            input_a: Some(operand.into()),
            input_b: None,
            operation: op,
            label: None,
        }
    }

    pub fn copy(
        to: MOIRLocalId,
        from: impl Into<MOIRLocal>,
    ) -> Self {
        MOIRInstruction {
            output: Some(to),
            input_a: Some(from.into()),
            input_b: None,
            operation: MOIROperation::Copy,
            label: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MOIROperation {
    /// Special build operation
    Build(TableId),
    Copy,
    CustomOp {
        op_id: CustomOpId,
        extra_args: Vec<MOIRLocal>,
    },
    /// This operation does nothing. Its inputs and outputs are ignored.
    Nop,

    // Special operations
    /// Special AddressOf operation. This is translated into a constant, when the constructor is called.
    AddressOf,
    PopCount,
    LZCount,
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
    Carry,
    SignCarry,
    SignBorrow,
    Nan,
    Abs,
    Sqrt,
    IntToFloat,
    FloatToFloat,
    Trunc,
    Ceil,
    Floor,
    Round,

    // Load and store operations
    // When these operations are resolved into JIR upon calling the constructor, all values
    // that refer to a `const` space are translated into actual constants, where their offset
    // is interpreted as the constants' value.
    Load(SpaceId),
    Store(SpaceId),

    // Branch instructions. Direct and Indirect Branch instructions use absolute addresses.
    // The label branch instruction uses a pcode relative label that is local to the scope.
    /// Branch to the EVAddr of the varnode input_a. The space of the varnode must be the same
    /// as the goto instruction.
    GotoDirect(MOIRGotoType),
    /// Branch to the EVAddr that is stored at the provided varnode input_a.
    GotoIndirect(MOIRGotoType),
    /// Branch to the scope local label specified in this operator.
    GotoLabel(MOIRGotoType, LabelId),
    GotoDirectIf,
    GotoIndirectIf,
    GotoLabelIf(LabelId),

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MOIRGotoType {
    Call,
    Return,
    Regular,
}

impl Display for MOIRGotoType {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRGotoType::Regular => write!(f, "goto"),
            MOIRGotoType::Call => write!(f, "call"),
            MOIRGotoType::Return => write!(f, "return"),
        }
    }
}

impl Display for MOIROperation {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIROperation::Build(table) => write!(f, "build {table}"),
            MOIROperation::Copy => write!(f, ""),
            MOIROperation::CustomOp { .. } => write!(f, "custom"),
            MOIROperation::AddressOf => write!(f, "&"),
            MOIROperation::PopCount => write!(f, "popcount"),
            MOIROperation::LZCount => write!(f, "lzcount"),
            MOIROperation::ZeroExtend => write!(f, "zext"),
            MOIROperation::SignExtend => write!(f, "sext"),
            MOIROperation::Carry => write!(f, "carry"),
            MOIROperation::SignCarry => write!(f, "scarry"),
            MOIROperation::SignBorrow => write!(f, "sborrow"),
            MOIROperation::Nan => write!(f, "nan"),
            MOIROperation::Abs => write!(f, "abs"),
            MOIROperation::Sqrt => write!(f, "sqrt"),
            MOIROperation::IntToFloat => write!(f, "int2float"),
            MOIROperation::FloatToFloat => write!(f, "float2float"),
            MOIROperation::Trunc => write!(f, "trunc"),
            MOIROperation::Ceil => write!(f, "ceil"),
            MOIROperation::Floor => write!(f, "floor"),
            MOIROperation::Round => write!(f, "round"),
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
            MOIROperation::GotoDirect(ty) | MOIROperation::GotoIndirect(ty) | MOIROperation::GotoLabel(ty, _) => {
                write!(f, "{ty}")
            }
            MOIROperation::GotoDirectIf | MOIROperation::GotoIndirectIf | MOIROperation::GotoLabelIf(_) => {
                write!(f, "goto")
            }
            MOIROperation::Nop => write!(f, "nop"),
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
            Build(_)
            | CustomOp { .. }
            | PopCount
            | LZCount
            | ZeroExtend
            | SignExtend
            | Carry
            | SignCarry
            | SignBorrow
            | Nan
            | Abs
            | Sqrt
            | IntToFloat
            | FloatToFloat
            | Trunc
            | Ceil
            | Floor
            | Round
            | Load(_)
            | Store(_)
            | AddressOf => MOIROpCategory::Special,
            GotoDirect(_) | GotoIndirect(_) | GotoDirectIf | GotoIndirectIf => {
                MOIROpCategory::Branch(MOIRBranchCategory::External)
            }
            GotoLabel(..) | GotoLabelIf(_) => MOIROpCategory::Branch(MOIRBranchCategory::Internal),
            _ => MOIROpCategory::Regular,
        }
    }
}

pub enum MOIRBranchCategory {
    Internal,
    External,
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
    /// Branching operations
    Branch(MOIRBranchCategory),
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
