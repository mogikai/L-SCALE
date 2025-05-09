use crate::spec::jir::jcode::JIRInstruction;
use std::fmt::{
    Display,
    Formatter,
};

impl Display for JIRInstruction {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        use JIRInstruction as OP;
        match &self {
            OP::BlockStart(label) => write!(f, "block_start <{}>", label),
            OP::BlockEnd(label) => write!(f, "block_end <{}>", label),
            OP::InstructionStart(addr) => write!(f, "instruction_start <{:?}>", addr),
            OP::InstructionEnd(addr) => write!(f, "instruction_end <{:?}>", addr),
            OP::PopCount(args)
            | OP::LZCount(args)
            | OP::ZeroExtend(args)
            | OP::SignExtend(args)
            | OP::Nan(args)
            | OP::Abs(args)
            | OP::Sqrt(args)
            | OP::IntToFloat(args)
            | OP::FloatToFloat(args)
            | OP::Trunc(args)
            | OP::Ceil(args)
            | OP::Floor(args)
            | OP::Round(args) => {
                write!(f, "{} = {}({})", args.output, self.op_display(), args.input)
            }
            OP::Carry(args) | OP::SignCarry(args) | OP::SignBorrow(args) => {
                write!(f, "{} = {}({}, {})", args.output, self.op_display(), args.input_a, args.input_b)
            }
            OP::Load { target, address, space } => {
                write!(f, "{} = *[{:?}]{}", target, space, address)
            }
            OP::Store { address, value, space } => {
                write!(f, "*[{:?}]{} = {}", space, address, value)
            }
            OP::Copy(args) => {
                write!(f, "{} = {}", args.output, args.input)
            }
            OP::CustomOp {
                output,
                input_a,
                input_b,
                op_id,
                extra_args,
            } => {
                if let Some(output) = output {
                    write!(f, "{} = {}(", output, op_id)?
                } else {
                    write!(f, "{}(", op_id)?
                }
                let mut inputs = vec![];
                if let Some(input) = input_a {
                    inputs.push(input);
                }
                if let Some(input) = input_b {
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
            OP::BoolNegate(args) | OP::BitNegate(args) | OP::IntNegate(args) | OP::FloatNegate(args) => {
                write!(f, "{} = {}{}", args.output, self.op_display(), args.input)
            }
            OP::ReturnDirect(target) => write!(f, "return {}", target),
            OP::ReturnIndirect(target) => write!(f, "return [{}]", target),
            OP::CallDirect(target, retaddr) => write!(f, "call {} (returning to {retaddr:?})", target),
            OP::CallIndirect(target, retaddr) => write!(f, "call [{}] (returning to {retaddr:?}", target),
            OP::GotoDirect(target) => write!(f, "goto {}", target),
            OP::GotoIndirect(target) => write!(f, "goto [{}]", target),
            OP::JCodeGoto(label) => write!(f, "goto <{}>", label),
            OP::JCodeGotoIf {
                condition,
                target_block,
            } => {
                write!(f, "if ({}) goto <{}>", condition, target_block)
            }
            OP::GotoDirectIf { condition, target } => write!(f, "if ({}) goto [{}]", condition, target),
            OP::GotoIndirectIf { condition, target } => write!(f, "if ({}) goto {}", condition, target),
            JIRInstruction::IntAdd(args)
            | JIRInstruction::IntSubtract(args)
            | JIRInstruction::IntMul(args)
            | JIRInstruction::IntDiv(args)
            | JIRInstruction::IntSDiv(args)
            | JIRInstruction::IntRemainder(args)
            | JIRInstruction::IntSRemainder(args)
            | JIRInstruction::IntLeft(args)
            | JIRInstruction::IntRight(args)
            | JIRInstruction::IntSRight(args)
            | JIRInstruction::IntOr(args)
            | JIRInstruction::IntAnd(args)
            | JIRInstruction::IntXor(args)
            | JIRInstruction::IntSLess(args)
            | JIRInstruction::IntSGreater(args)
            | JIRInstruction::IntSLessEqual(args)
            | JIRInstruction::IntSGreaterEqual(args)
            | JIRInstruction::IntLess(args)
            | JIRInstruction::IntGreater(args)
            | JIRInstruction::IntLessEqual(args)
            | JIRInstruction::IntGreaterEqual(args)
            | JIRInstruction::IntEquals(args)
            | JIRInstruction::IntNotEquals(args)
            | JIRInstruction::FloatDiv(args)
            | JIRInstruction::FloatMul(args)
            | JIRInstruction::FloatAdd(args)
            | JIRInstruction::FloatSubtract(args)
            | JIRInstruction::FloatLess(args)
            | JIRInstruction::FloatGreater(args)
            | JIRInstruction::FloatLessEqual(args)
            | JIRInstruction::FloatGreaterEqual(args)
            | JIRInstruction::FloatEquals(args)
            | JIRInstruction::FloatNotEquals(args)
            | JIRInstruction::BoolOr(args)
            | JIRInstruction::BoolAnd(args)
            | JIRInstruction::BoolXor(args) => {
                write!(f, "{} = {} {} {}", args.output, args.input_a, self.op_display(), args.input_b)
            }
            JIRInstruction::Nop => write!(f, "nop")
        }
    }
}

impl JIRInstruction {
    fn op_display(&self) -> &str {
        match self {
            JIRInstruction::BlockStart(_) => "block_start",
            JIRInstruction::BlockEnd(_) => "block_end",
            JIRInstruction::InstructionStart(_) => "instruction_start",
            JIRInstruction::InstructionEnd(_) => "instruction_end",
            JIRInstruction::Copy(_) => "",
            JIRInstruction::CustomOp { .. } => "custom",
            JIRInstruction::PopCount(_) => "popcount",
            JIRInstruction::LZCount(_) => "lzcount",
            JIRInstruction::ZeroExtend(_) => "zext",
            JIRInstruction::SignExtend(_) => "sext",
            JIRInstruction::Carry(_) => "carry",
            JIRInstruction::SignCarry(_) => "scarry",
            JIRInstruction::SignBorrow(_) => "sborrow",
            JIRInstruction::Nan(_) => "nan",
            JIRInstruction::Abs(_) => "abs",
            JIRInstruction::Sqrt(_) => "sqrt",
            JIRInstruction::IntToFloat(_) => "int2float",
            JIRInstruction::FloatToFloat(_) => "float2float",
            JIRInstruction::Trunc(_) => "trunc",
            JIRInstruction::Ceil(_) => "ceil",
            JIRInstruction::Floor(_) => "floor",
            JIRInstruction::Round(_) => "round",
            JIRInstruction::Load { .. } => "load",
            JIRInstruction::Store { .. } => "store",
            JIRInstruction::IntAdd(_) => "+",
            JIRInstruction::IntSubtract(_) => "-",
            JIRInstruction::IntMul(_) => "*",
            JIRInstruction::IntDiv(_) => "/",
            JIRInstruction::IntSDiv(_) => "s/",
            JIRInstruction::IntRemainder(_) => "%",
            JIRInstruction::IntSRemainder(_) => "s%",
            JIRInstruction::FloatDiv(_) => "f/",
            JIRInstruction::FloatMul(_) => "f*",
            JIRInstruction::FloatAdd(_) => "f+",
            JIRInstruction::FloatSubtract(_) => "f-",
            JIRInstruction::BoolOr(_) => "||",
            JIRInstruction::BoolAnd(_) => "&&",
            JIRInstruction::BoolXor(_) => "^^",
            JIRInstruction::IntLeft(_) => "<<",
            JIRInstruction::IntRight(_) => ">>",
            JIRInstruction::IntSRight(_) => "s>>",
            JIRInstruction::IntSLess(_) => "s<",
            JIRInstruction::IntSGreater(_) => "s>",
            JIRInstruction::IntSLessEqual(_) => "s<=",
            JIRInstruction::IntSGreaterEqual(_) => "s>=",
            JIRInstruction::IntLess(_) => "<",
            JIRInstruction::IntGreater(_) => ">",
            JIRInstruction::IntLessEqual(_) => "<=",
            JIRInstruction::IntGreaterEqual(_) => ">=",
            JIRInstruction::FloatLess(_) => "f<",
            JIRInstruction::FloatGreater(_) => "f>",
            JIRInstruction::FloatLessEqual(_) => "f<=",
            JIRInstruction::FloatGreaterEqual(_) => "f>=",
            JIRInstruction::FloatEquals(_) => "f==",
            JIRInstruction::FloatNotEquals(_) => "f!=",
            JIRInstruction::IntOr(_) => "|",
            JIRInstruction::IntAnd(_) => "&",
            JIRInstruction::IntXor(_) => "^",
            JIRInstruction::IntEquals(_) => "==",
            JIRInstruction::IntNotEquals(_) => "!=",
            JIRInstruction::BoolNegate(_) => "!",
            JIRInstruction::BitNegate(_) => "~",
            JIRInstruction::IntNegate(_) => "-",
            JIRInstruction::FloatNegate(_) => "f-",
            JIRInstruction::Nop => "nop",
            JIRInstruction::CallDirect(..) | JIRInstruction::CallIndirect(..) => "call",
            JIRInstruction::ReturnDirect(_) | JIRInstruction::ReturnIndirect(_) => "return",
            JIRInstruction::GotoDirect(_)
            | JIRInstruction::GotoIndirect(_)
            | JIRInstruction::GotoDirectIf { .. }
            | JIRInstruction::GotoIndirectIf { .. }
            | JIRInstruction::JCodeGoto(_)
            | JIRInstruction::JCodeGotoIf { .. } => "goto",
        }
    }
}
