use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::expression::{
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
};
use crate::compiler::build::atomize::expression::{
    from_bool,
    into_bool,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROperation,
    moir_instruction,
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
                    MOIRConst::Determined(operand, size) => {
                        MOIRConst::Determined(const_eval_unary_op(op.into_value(), operand)?, size).into()
                    }
                    MOIRConst::Undetermined(_) => {
                        todo!("Implement unary const folding with undetermined values or generate error?")
                    }
                })
            }
        }
    }
}

fn const_eval_unary_op(
    op: SASTPCodeUnaryOp,
    operand: u128,
) -> Result<u128, SleighCompilerError>
where
{
    Ok(match op {
        SASTPCodeUnaryOp::BoolNegate => from_bool(!into_bool(operand)),
        SASTPCodeUnaryOp::BitNegate => !operand,
        SASTPCodeUnaryOp::IntNegate => (-(operand as i128)) as u128,
        SASTPCodeUnaryOp::FloatNegate => todo!(),
    })
}
