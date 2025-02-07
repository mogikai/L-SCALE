use crate::spec::jir::jcode::{
    JIRInstruction,
    JIROperation,
};
use std::fmt::{
    Display,
    Formatter,
};

impl Display for JIRInstruction {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match &self.operation {
            JIROperation::BlockStart(label) => write!(f, "block_start <{}>", label),
            JIROperation::BlockEnd(label) => write!(f, "block_end <{}>", label),
            JIROperation::InstructionStart(addr) => write!(f, "instruction_start <{:?}>", addr),
            JIROperation::InstructionEnd(addr) => write!(f, "instruction_end <{:?}>", addr),
            JIROperation::PopCount
            | JIROperation::LZCount
            | JIROperation::ZeroExtend
            | JIROperation::SignExtend
            | JIROperation::Nan
            | JIROperation::Abs
            | JIROperation::Sqrt
            | JIROperation::IntToFloat
            | JIROperation::FloatToFloat
            | JIROperation::Trunc
            | JIROperation::Ceil
            | JIROperation::Floor
            | JIROperation::Round => {
                write!(
                    f,
                    "{} = {}({})",
                    self.output.expect("Extend ops have an output"),
                    self.operation,
                    self.input_a.expect("Extend ops have to have input_a")
                )
            }
            JIROperation::Carry | JIROperation::SignCarry | JIROperation::SignBorrow => {
                write!(
                    f,
                    "{} = {}({}, {})",
                    self.output.expect("op has to have an output"),
                    self.operation,
                    self.input_a.expect("op has to have input_a"),
                    self.input_b.expect("op has to have input_a")
                )
            }
            JIROperation::Load(space) => {
                write!(
                    f,
                    "{} = *[{:?}]{}",
                    self.output.expect("Load ops have an output"),
                    space,
                    self.input_a.expect("Load has to have input_a")
                )
            }
            JIROperation::Store(space) => {
                write!(
                    f,
                    "*[{:?}]{} = {}",
                    space,
                    self.input_a.expect("Store has to have input_a"),
                    self.input_b.expect("Store has to have input_b")
                )
            }
            JIROperation::Copy => {
                write!(
                    f,
                    "{} = {}",
                    self.output.expect("Copy ops have an output"),
                    self.input_a.expect("Copy has to have input_a")
                )
            }
            JIROperation::CustomOp { op_id, extra_args } => {
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
                inputs.extend(extra_args.iter());
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
            JIROperation::BoolNegate
            | JIROperation::BitNegate
            | JIROperation::IntNegate
            | JIROperation::FloatNegate => {
                write!(
                    f,
                    "{} = {}{}",
                    self.output.expect("Unary op has to have an output"),
                    self.operation,
                    self.input_a.expect("Unary op has to have input_a")
                )
            }
            JIROperation::ReturnDirect => write!(f, "return {}", self.input_a.expect("Goto has to have input_a")),
            JIROperation::ReturnIndirect => write!(f, "return [{}]", self.input_a.expect("Goto has to have input_a")),
            JIROperation::CallDirect => write!(f, "call {}", self.input_a.expect("Goto has to have input_a")),
            JIROperation::CallIndirect => write!(f, "call [{}]", self.input_a.expect("Goto has to have input_a")),
            JIROperation::GotoDirect => write!(f, "goto {}", self.input_a.expect("Goto has to have input_a")),
            JIROperation::GotoIndirect => write!(f, "goto [{}]", self.input_a.expect("Goto has to have input_a")),
            JIROperation::JCodeGoto(label) => write!(f, "goto <{}>", label),
            JIROperation::JCodeGotoIf(label) => {
                write!(f, "if ({}) goto <{:?}>", self.input_b.expect("GotoIf has to have input_b"), label)
            }
            JIROperation::GotoDirectIf => write!(
                f,
                "if ({}) goto [{}]",
                self.input_b.expect("GotoIf has to have input_b"),
                self.input_a.expect("Goto has to have input_a")
            ),
            JIROperation::GotoIndirectIf => write!(
                f,
                "if ({}) goto {}",
                self.input_b.expect("GotoIf has to have input_b"),
                self.input_a.expect("Goto has to have input_a")
            ),
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

impl Display for JIROperation {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            JIROperation::BlockStart(_) => write!(f, "block_start"),
            JIROperation::BlockEnd(_) => write!(f, "block_end"),
            JIROperation::InstructionStart(_) => write!(f, "instruction_start"),
            JIROperation::InstructionEnd(_) => write!(f, "instruction_end"),
            JIROperation::Copy => write!(f, ""),
            JIROperation::CustomOp { .. } => write!(f, "custom"),
            JIROperation::PopCount => write!(f, "popcount"),
            JIROperation::LZCount => write!(f, "lzcount"),
            JIROperation::ZeroExtend => write!(f, "zext"),
            JIROperation::SignExtend => write!(f, "sext"),
            JIROperation::Carry => write!(f, "carry"),
            JIROperation::SignCarry => write!(f, "scarry"),
            JIROperation::SignBorrow => write!(f, "sborrow"),
            JIROperation::Nan => write!(f, "nan"),
            JIROperation::Abs => write!(f, "abs"),
            JIROperation::Sqrt => write!(f, "sqrt"),
            JIROperation::IntToFloat => write!(f, "int2float"),
            JIROperation::FloatToFloat => write!(f, "float2float"),
            JIROperation::Trunc => write!(f, "trunc"),
            JIROperation::Ceil => write!(f, "ceil"),
            JIROperation::Floor => write!(f, "floor"),
            JIROperation::Round => write!(f, "round"),
            JIROperation::Load(_) => write!(f, "load"),
            JIROperation::Store(_) => write!(f, "store"),
            JIROperation::IntAdd => write!(f, "+"),
            JIROperation::IntSubtract => write!(f, "-"),
            JIROperation::IntMul => write!(f, "*"),
            JIROperation::IntDiv => write!(f, "/"),
            JIROperation::IntSDiv => write!(f, "s/"),
            JIROperation::IntRemainder => write!(f, "%"),
            JIROperation::IntSRemainder => write!(f, "s%"),
            JIROperation::FloatDiv => write!(f, "f/"),
            JIROperation::FloatMul => write!(f, "f*"),
            JIROperation::FloatAdd => write!(f, "f+"),
            JIROperation::FloatSubtract => write!(f, "f-"),
            JIROperation::BoolOr => write!(f, "||"),
            JIROperation::BoolAnd => write!(f, "&&"),
            JIROperation::BoolXor => write!(f, "^^"),
            JIROperation::IntLeft => write!(f, "<<"),
            JIROperation::IntRight => write!(f, ">>"),
            JIROperation::IntSRight => write!(f, "s>>"),
            JIROperation::IntSLess => write!(f, "s<"),
            JIROperation::IntSGreater => write!(f, "s>"),
            JIROperation::IntSLessEqual => write!(f, "s<="),
            JIROperation::IntSGreaterEqual => write!(f, "s>="),
            JIROperation::IntLess => write!(f, "<"),
            JIROperation::IntGreater => write!(f, ">"),
            JIROperation::IntLessEqual => write!(f, "<="),
            JIROperation::IntGreaterEqual => write!(f, ">="),
            JIROperation::FloatLess => write!(f, "f<"),
            JIROperation::FloatGreater => write!(f, "f>"),
            JIROperation::FloatLessEqual => write!(f, "f<="),
            JIROperation::FloatGreaterEqual => write!(f, "f>="),
            JIROperation::FloatEquals => write!(f, "f=="),
            JIROperation::FloatNotEquals => write!(f, "f!="),
            JIROperation::IntOr => write!(f, "|"),
            JIROperation::IntAnd => write!(f, "&"),
            JIROperation::IntXor => write!(f, "^"),
            JIROperation::IntEquals => write!(f, "=="),
            JIROperation::IntNotEquals => write!(f, "!="),
            JIROperation::BoolNegate => write!(f, "!"),
            JIROperation::BitNegate => write!(f, "~"),
            JIROperation::IntNegate => write!(f, "-"),
            JIROperation::FloatNegate => write!(f, "f-"),
            JIROperation::CallDirect | JIROperation::CallIndirect => write!(f, "call"),
            JIROperation::ReturnDirect | JIROperation::ReturnIndirect => write!(f, "return"),
            JIROperation::GotoDirect
            | JIROperation::GotoIndirect
            | JIROperation::GotoDirectIf
            | JIROperation::GotoIndirectIf
            | JIROperation::JCodeGoto(_)
            | JIROperation::JCodeGotoIf(_) => {
                write!(f, "goto")
            }
        }
    }
}
