use crate::compiler::build::atomize::expression::{
    from_bool,
    into_bool,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::{
    ConstEvalError,
    InternalError,
};
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
    SASTPcodeOpCategory,
};

impl SleighCompiler {
    pub fn atomize_binary_operation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        ast_lhs: Spanned<SASTPCodeExpression>,
        op: Spanned<SASTPCodeBinaryOp>,
        ast_rhs: Spanned<SASTPCodeExpression>,
    ) -> InternalCompilerResult<MOIRLocal> {
        let mut independent_rhs_size = false;
        let (out, lhs) = match op.value().op_category() {
            SASTPcodeOpCategory::Comparison => {
                // If op is a comparison operation, it will output either 0 or 1, which is put into a single byte.
                // See 7.7.3. Varnode Sizes.
                // The output is constrained to a single byte.
                let out = out.unwrap_or_else(|| scope.allocate_temporary(MOIRMaybeSize::Determined(1)));
                match scope.size_of_local(out) {
                    MOIRMaybeSize::Determined(1) => {}
                    MOIRMaybeSize::Determined(_) => {
                        todo!("Generate error for invalid size for comparison operation. Out must be size 1")
                    }
                    MOIRMaybeSize::Undetermined => {
                        // The size of out was undetermined, so we can just constrain it now.
                        let mut out = out.into();
                        scope.constrain_local_size(&mut out, 1)?;
                    }
                }
                (out, None)
            }
            SASTPcodeOpCategory::Shift => {
                // The rhs for shift operations is unconstrained.
                independent_rhs_size = true;
                let out = out.unwrap_or_else(|| scope.allocate_temporary(MOIRMaybeSize::Undetermined));
                // Use the same temporary for out and lhs since they have to be the same size.
                (out, Some(out))
            }
            SASTPcodeOpCategory::Regular => {
                let out = out.unwrap_or_else(|| scope.allocate_temporary(MOIRMaybeSize::Undetermined));
                // Use the same temporary for out and lhs since they have to be the same size.
                (out, Some(out))
            }
        };

        let lhs = self.atomize_expression(scope, instructions, lhs, ast_lhs)?;

        // The size of the rhs must be the same as the lhs. Since size_constraint was given to lhs,
        // we can now just assume lhs has a determined size if size_constraint had a determined size.
        // The exception to this are shift operations, where th rhs is independent of the lhs.
        let rhs_size = if independent_rhs_size {
            MOIRMaybeSize::Undetermined
        } else {
            lhs.size(scope)
        };
        // Rhs needs to be allocated separately, since we need to keep both intermediate values at the same time
        // However, we can directly allocate the correct size.
        let rhs = scope.allocate_temporary(rhs_size);
        let rhs = self.atomize_expression(scope, instructions, Some(rhs), ast_rhs)?;

        if let MOIRLocal::Const(lhs) = lhs {
            if let MOIRLocal::Const(rhs) = rhs {
                trace_build!("Performing const optimization since lhs and rhs are const");
                // Constant folding. We can calculate this at compile time.
                // TODO better error variant for const eval
                return const_eval_binary_op(lhs, op.into_value(), rhs)
                    .map(MOIRLocal::Const)
                    // TODO ues span_err!
                    .map_err(|e| InternalError::from_error(e.into()));
            }
        }

        // If out is not yet determined, try to constrain its size now.
        if let MOIRMaybeSize::Undetermined = scope.size_of_local(out) {
            match op.value().op_category() {
                SASTPcodeOpCategory::Comparison => {
                    let mut out = out.into();
                    scope.constrain_local_size(&mut out, 1)?;
                }
                SASTPcodeOpCategory::Shift | SASTPcodeOpCategory::Regular => {
                    let mut out = out.into();
                    if let MOIRMaybeSize::Determined(lhs_size) = lhs.size(scope) {
                        scope.constrain_local_size(&mut out, lhs_size)?;
                    }
                }
            }
        }

        // At least one side of the op is not const, so we cannot fold.
        let op = op.into_value().into();
        trace_build!("Sizes: {:?} = {:?} op {:?}", scope.size_of_local(out), lhs.size(scope), rhs.size(scope));
        instructions.push(moir_instruction!(out = lhs op rhs));
        Ok(out.into())
    }
}

fn const_eval_binary_op(
    lhs: MOIRConst,
    op: SASTPCodeBinaryOp,
    rhs: MOIRConst,
) -> Result<MOIRConst, ConstEvalError> {
    match lhs {
        MOIRConst::Determined(lhs, lhs_size) => {
            let MOIRConst::Determined(rhs, rhs_size) = rhs else {
                todo!("Generate error report for undetermined size")
            };
            if lhs_size != rhs_size {
                todo!("Generate error report for incorrect sizes")
            }
            Ok(MOIRConst::Determined(const_eval_binary_op_generic(lhs, op, rhs)?, lhs_size))
        }
        MOIRConst::Undetermined(_) => {
            todo!("Cannot const evaluate undetermined size constants")
        }
    }
}

fn const_eval_binary_op_generic(
    lhs: u128,
    op: SASTPCodeBinaryOp,
    rhs: u128,
) -> Result<u128, ConstEvalError>
where
{
    trace_build!("Const eval for {lhs} {op:?} {rhs}");
    Ok(match op {
        SASTPCodeBinaryOp::IntAdd => lhs.checked_add(rhs).ok_or(ConstEvalError::Overflow)?,
        SASTPCodeBinaryOp::IntSubtract => lhs.checked_sub(rhs).ok_or(ConstEvalError::Overflow)?,
        SASTPCodeBinaryOp::IntMul => lhs * rhs,
        SASTPCodeBinaryOp::IntDiv => lhs / rhs,
        SASTPCodeBinaryOp::IntSDiv => todo!(),
        SASTPCodeBinaryOp::IntRemainder => todo!(),
        SASTPCodeBinaryOp::IntSRemainder => todo!(),
        SASTPCodeBinaryOp::FloatDiv => todo!(),
        SASTPCodeBinaryOp::FloatMul => todo!(),
        SASTPCodeBinaryOp::FloatAdd => todo!(),
        SASTPCodeBinaryOp::FloatSubtract => todo!(),
        SASTPCodeBinaryOp::BoolOr => from_bool(into_bool(lhs) || into_bool(rhs)),
        SASTPCodeBinaryOp::BoolAnd => from_bool(into_bool(lhs) && into_bool(rhs)),
        SASTPCodeBinaryOp::BoolXor => from_bool(into_bool(lhs) ^ into_bool(rhs)),
        SASTPCodeBinaryOp::IntLeft => lhs << rhs,
        SASTPCodeBinaryOp::IntRight => lhs >> rhs,
        SASTPCodeBinaryOp::IntSRight => todo!(),
        SASTPCodeBinaryOp::IntSLess => todo!(),
        SASTPCodeBinaryOp::IntSGreater => todo!(),
        SASTPCodeBinaryOp::IntSLessEqual => todo!(),
        SASTPCodeBinaryOp::IntSGreaterEqual => todo!(),
        SASTPCodeBinaryOp::IntLess => todo!(),
        SASTPCodeBinaryOp::IntGreater => todo!(),
        SASTPCodeBinaryOp::IntLessEqual => todo!(),
        SASTPCodeBinaryOp::IntGreaterEqual => todo!(),
        SASTPCodeBinaryOp::FloatLess => todo!(),
        SASTPCodeBinaryOp::FloatGreater => todo!(),
        SASTPCodeBinaryOp::FloatLessEqual => todo!(),
        SASTPCodeBinaryOp::FloatGreaterEqual => todo!(),
        SASTPCodeBinaryOp::FloatEquals => todo!(),
        SASTPCodeBinaryOp::FloatNotEquals => todo!(),
        SASTPCodeBinaryOp::IntOr => lhs | rhs,
        SASTPCodeBinaryOp::IntAnd => lhs & rhs,
        SASTPCodeBinaryOp::IntXor => lhs ^ rhs,
        SASTPCodeBinaryOp::IntEquals => from_bool(lhs == rhs),
        SASTPCodeBinaryOp::IntNotEquals => from_bool(lhs != rhs),
    })
}
