//! This module implements the atomization of PCodeExpressions.
//! The entry function is [SleighCompiler::atomize_expression].

use crate::compiler::ast::constructor::bit_pattern_section::SASTBinaryOp;
use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
};
use crate::compiler::ast::Spanned;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    MOIRConst,
    MOIRInstruction,
    MOIRValue,
};
use std::fmt::Display;
use std::ops::{
    Add,
    BitAnd,
    BitOr,
    BitXor,
    Div,
    Mul,
    Shl,
    Shr,
    Sub,
};
use tracing::trace;

impl SleighCompiler {
    pub(super) fn atomize_binary_operation(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        lhs: Spanned<SASTPCodeExpression>,
        op: Spanned<SASTPCodeBinaryOp>,
        rhs: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRValue> {
        let lhs = self.atomize_expression(instructions, lhs)?;
        let rhs = self.atomize_expression(instructions, rhs)?;
        if let MOIRValue::Const(lhs) = lhs {
            if let MOIRValue::Const(rhs) = rhs {
                trace!("Performing const optimization since lhs and rhs are const");
                // Constant folding. We can calculate this at compile time.
                return self.const_eval_binary_op(lhs, op.into_value(), rhs);
            }
        }
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    fn const_eval_binary_op(
        &mut self,
        lhs: MOIRConst,
        op: SASTPCodeBinaryOp,
        rhs: MOIRConst,
    ) -> SleighCompilerResult<MOIRValue> {
        match lhs {
            MOIRConst::Const64(lhs) => {
                let MOIRConst::Const64(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const64(const_eval_binary_op(lhs, op, rhs)))
            }
            MOIRConst::Const32(lhs) => {
                let MOIRConst::Const32(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const32(const_eval_binary_op(lhs, op, rhs)))
            }
            MOIRConst::Const16(lhs) => {
                let MOIRConst::Const16(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const16(const_eval_binary_op(lhs, op, rhs)))
            }
            MOIRConst::Const8(lhs) => {
                let MOIRConst::Const8(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const8(const_eval_binary_op(lhs, op, rhs)))
            }
        }
        .map(MOIRValue::Const)
    }

    pub(super) fn atomize_unary_operation(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_dereference(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_subpiece(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_bitrange(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_identifier(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_load(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_address_of(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }

    pub(super) fn atomize_literal(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        literal: Spanned<u64>,
    ) -> SleighCompilerResult<MOIRValue> {
        Ok(MOIRValue::Const(MOIRConst::Const64(literal.into_value())))
    }

    pub(super) fn atomize_error(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO
        Ok(MOIRValue::Const(MOIRConst::Const64(0)))
    }
}

fn const_eval_binary_op<T>(
    lhs: T,
    op: SASTPCodeBinaryOp,
    rhs: T,
) -> T
where
    T: Add<Output = T>,
    T: Sub<Output = T>,
    T: Mul<Output = T>,
    T: Div<Output = T>,
    T: Shr<Output = T>,
    T: Shl<Output = T>,
    T: BitAnd<Output = T>,
    T: BitOr<Output = T>,
    T: BitXor<Output = T>,
    T: Display,
{
    trace!("Const eval for {lhs} {op:?} {rhs}");
    // match op {
    //     SASTPCodeBinaryOp::IntAdd => lhs + rhs,
    //     SASTPCodeBinaryOp::IntSubtract => lhs - rhs,
    //     SASTPCodeBinaryOp::IntMul => lhs * rhs,
    //     SASTPCodeBinaryOp::IntDiv => lhs / rhs,
    //     SASTPCodeBinaryOp::IntSDiv => todo!(),
    //     SASTPCodeBinaryOp::IntRemainder => todo!(),
    //     SASTPCodeBinaryOp::IntSRemainder => todo!(),
    //     SASTPCodeBinaryOp::FloatDiv => todo!(),
    //     SASTPCodeBinaryOp::FloatMul => todo!(),
    //     SASTPCodeBinaryOp::FloatAdd => todo!(),
    //     SASTPCodeBinaryOp::FloatSubtract => todo!(),
    //     SASTPCodeBinaryOp::BoolOr => todo!(),
    //     SASTPCodeBinaryOp::BoolAnd => todo!(),
    //     SASTPCodeBinaryOp::BoolXor => todo!(),
    //     SASTPCodeBinaryOp::IntLeft => lhs << rhs,
    //     SASTPCodeBinaryOp::IntRight => lhs >> rhs,
    //     SASTPCodeBinaryOp::IntSRight => todo!(),
    //     SASTPCodeBinaryOp::IntSLess => todo!(),
    //     SASTPCodeBinaryOp::IntSGreater => todo!(),
    //     SASTPCodeBinaryOp::IntSLessEqual => todo!(),
    //     SASTPCodeBinaryOp::IntSGreaterEqual => todo!(),
    //     SASTPCodeBinaryOp::IntLess => todo!(),
    //     SASTPCodeBinaryOp::IntGreater => todo!(),
    //     SASTPCodeBinaryOp::IntLessEqual => todo!(),
    //     SASTPCodeBinaryOp::IntGreaterEqual => todo!(),
    //     SASTPCodeBinaryOp::FloatLess => todo!(),
    //     SASTPCodeBinaryOp::FloatGreater => todo!(),
    //     SASTPCodeBinaryOp::FloatLessEqual => todo!(),
    //     SASTPCodeBinaryOp::FloatGreaterEqual => todo!(),
    //     SASTPCodeBinaryOp::FloatEquals => todo!(),
    //     SASTPCodeBinaryOp::FloatNotEquals => todo!(),
    //     SASTPCodeBinaryOp::IntOr => lhs | rhs,
    //     SASTPCodeBinaryOp::IntAnd => lhs & rhs,
    //     SASTPCodeBinaryOp::IntXor => lhs ^ rhs,
    //     SASTPCodeBinaryOp::IntEquals => todo!(),
    //     SASTPCodeBinaryOp::IntNotEquals => todo!(),
    // };
    lhs
}
