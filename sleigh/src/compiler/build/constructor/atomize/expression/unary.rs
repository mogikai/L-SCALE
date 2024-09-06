use crate::compiler::ast::pcode::expression::{
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::build::constructor::atomize::expression::{
    from_bool,
    into_bool,
    Cast,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};
use std::fmt::Display;
use std::ops::{
    Neg,
    Not,
};

impl SleighCompiler {
    pub fn atomize_unary_operation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        op: Spanned<SASTPCodeUnaryOp>,
        operand: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let out = self.atomize_expression(scope, instructions, out, operand)?;
        match out {
            MOIRLocal::Local(out) => {
                let op = MOIROperation::from(op.into_value());
                instructions.push(moir_instruction!(out = op out));
                Ok(out.into())
            }
            MOIRLocal::Const(operand) => {
                // We can do constant folding.
                Ok(match operand {
                    MOIRConst::ConstN(..) => {
                        todo!("Constant folding for ConstN currently unsupported")
                    }
                    MOIRConst::Const128(operand) => const_eval_unary_op::<u128, i128>(op.into_value(), operand)?.into(),
                    MOIRConst::Const64(operand) => const_eval_unary_op::<u64, i64>(op.into_value(), operand)?.into(),
                    MOIRConst::Const32(operand) => const_eval_unary_op::<u32, i32>(op.into_value(), operand)?.into(),
                    MOIRConst::Const16(operand) => const_eval_unary_op::<u16, i16>(op.into_value(), operand)?.into(),
                    MOIRConst::Const8(operand) => const_eval_unary_op::<u8, i8>(op.into_value(), operand)?.into(),
                    MOIRConst::Undetermined(_) => {
                        todo!("Implement unary const folding with undetermined values or generate error?")
                    }
                })
            }
        }
    }
}

fn const_eval_unary_op<T, S>(
    op: SASTPCodeUnaryOp,
    operand: T,
) -> Result<T, SleighCompilerError>
where
    T: Cast<S>,
    T: Not<Output = T>,
    T: Into<u128>,
    T: From<u8>,
    T: Display,
    S: Neg<Output = S>,
    S: Cast<T>,
{
    Ok(match op {
        SASTPCodeUnaryOp::BoolNegate => from_bool(!into_bool(operand)),
        SASTPCodeUnaryOp::BitNegate => !operand,
        SASTPCodeUnaryOp::IntNegate => (-operand.cast()).cast(),
        SASTPCodeUnaryOp::FloatNegate => todo!(),
    })
}
