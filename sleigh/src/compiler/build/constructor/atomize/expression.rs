//! This module implements the atomization of PCodeExpressions.
//! The entry function is [SleighCompiler::atomize_expression].

use std::cmp::Ordering;
use crate::compiler::ast::constructor::bit_pattern_section::SASTBinaryOp;
use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::{
    SASTPCodeBinaryOp,
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
    SASTPcodeOpCategory,
};
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::report::{
    generate_cannot_slice_error,
    generate_const_eval_error,
    generate_invalid_sizes_error,
};
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRSlice,
    MOIRVarNodeId,
};
use num::{
    CheckedAdd,
    CheckedSub,
    Num,
};
use std::fmt::Display;
use std::ops::{
    Add,
    BitAnd,
    BitOr,
    BitXor,
    Div,
    Mul,
    Neg,
    Not,
    Shl,
    Shr,
    Sub,
};
use tracing::trace;
use crate::compiler::ast::common::SASTLiteral;
use crate::spec::moir::address_space::MOIRSpaceType;

impl SleighCompiler {
    pub(super) fn atomize_binary_operation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        mut size_constraint: MOIRMaybeSize,
        lhs: Spanned<SASTPCodeExpression>,
        op: Spanned<SASTPCodeBinaryOp>,
        rhs: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let lhs_span = lhs.span().clone();
        let rhs_span = rhs.span().clone();
        let mut independent_rhs_size = false;

        match op.value().op_category() {
            SASTPcodeOpCategory::Comparison => {
                // If op is a comparison operation, it will output either 0 or 1, which is put into a single byte.
                // See 7.7.3. Varnode Sizes.
                // We set the size to undetermined, to allow the lhs and rhs to resolve to fitting values.
                // The output however, will be constrained to a single byte.
                size_constraint = MOIRMaybeSize::Undetermined;
            }
            SASTPcodeOpCategory::Shift => {
                // The rhs for shift operations is unconstrained.
                independent_rhs_size = true;
            }
            SASTPcodeOpCategory::Regular => {}
        }
        let lhs = self.atomize_expression(scope, instructions, size_constraint, lhs)?;
        // The size of the rhs must be the same as the lhs. Since size_constraint was given to lhs,
        // we can now just assume lhs has a determined size if size_constraint had a determined size.
        // The exception to this are shift operations, where th rhs is independent of the lhs.
        let rhs_size = if independent_rhs_size {
            MOIRMaybeSize::Undetermined
        } else {
            lhs.size(scope)
        };
        let rhs = self.atomize_expression(scope, instructions, rhs_size, rhs)?;
        // rhs.size must be equal to lhs.size at this point. Either both undefined or both the same size.
        if let MOIRLocal::Const(lhs) = lhs {
            if let MOIRLocal::Const(rhs) = rhs {
                trace!("Performing const optimization since lhs and rhs are const");
                // Constant folding. We can calculate this at compile time.
                return self
                    .const_eval_binary_op(lhs, op.into_value(), rhs)
                    .map_err(|e| generate_const_eval_error(e.to_string().as_str(), lhs_span, rhs_span));
            }
        }

        // At least one side of the op is not const, so we cannot fold.
        let out = if let SASTPcodeOpCategory::Comparison = op.value().op_category() {
            // if we have a comparison operation, the size of out is always one byte.
            // See 7.7.3 Varnode Sizes.
            scope.allocate_temporary(MOIRMaybeSize::Determined(1))
        } else {
            // if lhs and rhs depend on each other, then lhs must have the correct size.
            // if they don't depend on each other, then lhs is the only relevant size to consider.
            scope.allocate_temporary(lhs.size(scope))
        };
        let op = op.into_value().into();
        instructions.push(moir_instruction!(out = lhs op rhs));
        Ok(out.into())
    }

    fn const_eval_binary_op(
        &mut self,
        lhs: MOIRConst,
        op: SASTPCodeBinaryOp,
        rhs: MOIRConst,
    ) -> SleighCompilerResult<MOIRLocal> {
        match lhs {
            MOIRConst::ConstN(v, size) => {
                todo!("Const eval on ConstN currently unsupported")
            }
            MOIRConst::Const128(lhs) => {
                let MOIRConst::Const128(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const128(const_eval_binary_op(lhs, op, rhs)?))
            }
            MOIRConst::Const64(lhs) => {
                let MOIRConst::Const64(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const64(const_eval_binary_op(lhs, op, rhs)?))
            }
            MOIRConst::Const32(lhs) => {
                let MOIRConst::Const32(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const32(const_eval_binary_op(lhs, op, rhs)?))
            }
            MOIRConst::Const16(lhs) => {
                let MOIRConst::Const16(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const16(const_eval_binary_op(lhs, op, rhs)?))
            }
            MOIRConst::Const8(lhs) => {
                let MOIRConst::Const8(rhs) = rhs else {
                    todo!("Generate error report for incorrect sizes")
                };
                Ok(MOIRConst::Const8(const_eval_binary_op(lhs, op, rhs)?))
            }
            MOIRConst::Undetermined(_) => {
                todo!("Cannot const evaluate undetermined size constants")
            }
        }
        .map(MOIRLocal::Const)
    }

    pub(super) fn atomize_unary_operation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        op: Spanned<SASTPCodeUnaryOp>,
        operand: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let rhs = self.atomize_expression(scope, instructions, size_constraint, operand)?;
        match rhs {
            MOIRLocal::Local(rhs) => {
                // rhs is a value slice. Generate an instruction to process the value with op.
                // If rhs is determined, use its size. Otherwise, use the size_constraint.
                let out = scope.allocate_temporary(scope.size_of_local(rhs).or(size_constraint));
                let op = MOIROperation::from(op.into_value());
                instructions.push(moir_instruction!(out = op rhs));
                Ok(out.into())
            }
            MOIRLocal::Const(operand) => {
                // We can do constant folding.
                Ok(match operand {
                    MOIRConst::ConstN(operand, size) => {
                        todo!("Constant folding for ConstN currently unsupported")
                    }
                    MOIRConst::Const128(operand) => {
                        const_eval_unary_op::<u128, i128>(op.into_value(), operand)?.into()
                    }
                    MOIRConst::Const64(operand) => {
                        const_eval_unary_op::<u64, i64>(op.into_value(), operand)?.into()
                    }
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

    pub(super) fn atomize_dereference(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRLocal> {
        todo!()
    }

    /// The subpiece operation throws away the lower N bits.
    /// The resulting size of the output is thus (size - N).
    pub(super) fn atomize_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (subpiece, subpiece_span) = subpiece.into_tuple();
        let (offset, size) = match subpiece.selector.into_value() {
            SASTPCodeSubpieceSelector::Take(size) => {
                // If we take n, the offset is zero, and the size is determined as n
                (0, size)
            }
            SASTPCodeSubpieceSelector::Discard(offset) => {
                // If we discard n, then the offset is n and the size is undetermined,
                // i.e. must be determined by the external size constraint.
                (
                    offset,
                    size_constraint.ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error for missing size constraint")
                    })?,
                )
            }
        };
        let mut value = match subpiece.target.into_value() {
            SASTPCodeSubpieceTarget::Ident(ident) => {
                // We provide undetermined size here, because we do not want to constrain the underlying value.
                // We can also not create a subpiece, if the underlying value has undetermined size.
                self.atomize_identifier(scope, MOIRMaybeSize::Undetermined, ident.transmute())
            }
            SASTPCodeSubpieceTarget::Literal(literal) => {
                // For literal resolution, a size is provided, since we want to make sure that the value of which
                // the size is specified fits.
                self.atomize_literal(size.into(), literal)
            }
        }?;
        // If the size is not determined yet, we can constrain it here to the same size as the slice.
        if value.size(scope).is_undetermined() {
            scope.constrain_local_size(&mut value, size)?;
        }
        // We can now slice the value to the desired size.
        // TODO: Generate error when slice fails because of too large sizes.
        value.slice(scope, offset, offset + size)
    }

    pub(super) fn atomize_bitrange(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        bitrange: Spanned<SASTPCodeBitRange>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (bitrange, bitrange_scope) = bitrange.into_tuple();
        let value = scope.resolve_ident(&self.spec_builder, bitrange.ident.transmute())?;
        let shift_amount = MOIRLocal::from(bitrange.lsb.into_value());
        let bits = bitrange.bits.into_value();
        // The size of the output is the smallest size in bytes that still fits the output.
        let size = ((bits + 7) / 8).into();

        // Bitrange is an emulated instruction. We first shift the bits right until we have the lsb at position 0
        let int_right = MOIROperation::IntRight;
        let out = scope.allocate_temporary(MOIRMaybeSize::from(size));
        instructions.push(moir_instruction!(out = value int_right shift_amount));

        // We can now reduce the size of the slice, discarding the remaining bits.
        let out = scope.truncate_local(out, size)?;

        // And then mask out the bits that remain which we don't need.
        let int_and = MOIROperation::IntAnd;
        // Truncate the mask to correct size before storing it in a local
        let mask = MOIRLocal::from(MOIRConst::from((1u128 << bits) - 1).truncate(size)?);
        instructions.push(moir_instruction!(out = out int_and mask));
        Ok(out.into())
    }

    pub(super) fn atomize_identifier(
        &mut self,
        scope: &mut MOIRScope,
        size_constraint: MOIRMaybeSize,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing identifier with size_constraint {size_constraint:?}");
        let local = match size_constraint {
            MOIRMaybeSize::Determined(size) => {
                // The size constraint is determined. Update the size in the scope.
                scope
                    .resolve_ident_sized(&self.spec_builder, ident.clone(), size)
                    .map_err(|e| match e {
                        // TODO: Refactor into error helper function and generate better reports
                        SleighCompilerError::InvalidSliceSize { new, current } => {
                            generate_invalid_sizes_error(ident.into_span(), current, new)
                        }
                        SleighCompilerError::InvalidSliceRange {
                            current_start,
                            current_end,
                            new_start,
                            new_end,
                        } => generate_invalid_sizes_error(
                            ident.into_span(),
                            current_end - current_start,
                            new_end - new_start,
                        ),
                        SleighCompilerError::CannotSliceUndetermined => {
                            generate_invalid_sizes_error(ident.into_span(), 0, size)
                        }
                        e => e,
                    })?
            }
            MOIRMaybeSize::Undetermined => {
                // Size constraint is undetermined. We can just return the resolved value.
                scope.resolve_ident(&self.spec_builder, ident)?.into()
            }
        };
        Ok(local.into())
    }

    pub(super) fn atomize_load(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing load expression");
        let (deref, deref_span) = deref.into_tuple();
        let value_size = deref
                .size
                .map(Spanned::into_value)
                .map(MOIRMaybeSize::Determined)
                .unwrap_or(MOIRMaybeSize::Undetermined);
        // We want to first try and use the output size. If it is undetermined, try to use the size specified in deref.
        let out = scope.allocate_temporary(size_constraint.or(value_size));
        // TODO: Maybe we can do this less dirty?
        let space_id = deref
            .space
            .map(Spanned::transmute)
            .ok_or(|| SleighCompilerError::SymbolUndeclared)
            .or_else(|_| self.spec_builder.default_space_ident())?;
        let space = self.spec_builder.space_id_by_ident(space_id)?;
        let addr = self.atomize_expression(scope, instructions, MOIRMaybeSize::Undetermined, *deref.expression)?;
        if let MOIRMaybeSize::Determined(out_size ) = size_constraint {
            if let MOIRMaybeSize::Determined(value_size) = value_size {
                match out_size.cmp(&value_size) {
                    Ordering::Less => {todo!("Generate error for out too small for loaded value")}
                    Ordering::Equal => {
                        // If the size of out is equal to the size of the load, we can just perform the load.
                        instructions.push(moir_instruction!(out = *[space]addr));
                    }
                    Ordering::Greater => {
                        // If the size of out is larger than the loaded value, we perform a zero extend.
                        // Insert the operation here. The jit might optimize the zero extend + load into one op.
                        // Allocate tmp for loading the value into a correctly sized varnode.
                        let tmp = scope.allocate_temporary(value_size.into());
                        // Load the value
                        instructions.push(moir_instruction!(tmp = *[space]addr));
                        // Convert localid into local and then push zext instruction.
                        let tmp = tmp.into();
                        instructions.push(moir_instruction!(out = zext(tmp)));
                    }
                }
            }
        }
        Ok(out.into())
    }

    /// Translate the address_of operation. The sleigh specification allows taking the
    /// address of a varnode in the const space. This however translates directly
    /// to just providing the constant value, since the sleigh spec says that the offset
    /// of a varnode in the const address space corresponds to its value.
    /// I.e. the address of varnode {space: id_const, offset: 42, size: 4} translates to
    /// the constant value 42 with size 4.
    /// The address of and deref operations on values in the const address space or on
    /// constant values are translated on constructor invocation into their constant values.
    ///
    /// Quote from pcoderef.html:
    /// If a varnode is specified as an offset into the constant address space, that
    /// offset is interpreted as a constant, or immediate value, in any p-code operation
    /// that uses that varnode. The size of the varnode, in this case, can be treated
    /// as the size or precision available for the encoding of the constant.
    pub(super) fn atomize_address_of(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        address_of: Spanned<SASTPCodeAddressOf>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (address_of, _) = address_of.into_tuple();
        let size = address_of
            .size
            .map(|s| MOIRMaybeSize::Determined(s.into_value()))
            .unwrap_or(MOIRMaybeSize::Undetermined);
        if let MOIRMaybeSize::Determined(size_constraint) = size_constraint {
            // If the constraint is specified and the address_of operator also specifies
            // a size, they have to match.
            if let MOIRMaybeSize::Determined(size) = size {
                if size != size_constraint {
                    todo!("Generate error report for mismatched sizes")
                }
            }
        }
        let size = size_constraint.or(size);
        let lhs = scope.allocate_temporary(size);
        let rhs = self.atomize_expression(scope, instructions, size, *address_of.expression)?;
        instructions.push(moir_instruction!(lhs = &rhs));
        Ok(lhs.into())
    }

    pub(super) fn atomize_literal(
        &mut self,
        size_constraint: MOIRMaybeSize,
        literal: Spanned<SASTLiteral>,
    ) -> SleighCompilerResult<MOIRLocal> {
        Ok(MOIRConst::new_with_size(literal.into_value(), size_constraint)?.into())
    }

    pub(super) fn atomize_error(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRLocal> {
        // TODO
        Ok(MOIRLocal::Const(MOIRConst::Const64(0)))
    }
}

fn const_eval_binary_op<T>(
    lhs: T,
    op: SASTPCodeBinaryOp,
    rhs: T,
) -> Result<T, SleighCompilerError>
where
    T: CheckedAdd<Output = T>,
    T: CheckedSub<Output = T>,
    T: Mul<Output = T>,
    T: Div<Output = T>,
    T: Shr<Output = T>,
    T: Shl<Output = T>,
    T: BitAnd<Output = T>,
    T: BitOr<Output = T>,
    T: BitXor<Output = T>,
    T: PartialEq<T>,
    T: Into<u128>,
    T: From<u8>,
    T: Display,
{
    trace!("Const eval for {lhs} {op:?} {rhs}");
    Ok(match op {
        SASTPCodeBinaryOp::IntAdd => lhs
            .checked_add(&rhs)
            .ok_or(SleighCompilerError::ConstEvalOverflow)?,
        SASTPCodeBinaryOp::IntSubtract => lhs
            .checked_sub(&rhs)
            .ok_or(SleighCompilerError::ConstEvalOverflow)?,
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

fn into_bool<T>(val: T) -> bool
where
    T: Into<u128>,
{
    match val.into() {
        0u128 => false,
        _ => true,
    }
}

fn from_bool<T>(val: bool) -> T
where
    T: From<u8>,
{
    match val {
        false => 0u8.into(),
        true => 1.into(),
    }
}

trait Cast<T> {
    fn cast(self) -> T;
}

macro_rules! impl_cast {
    ($from:ty, $to:ty) => {
        impl Cast<$to> for $from {
            fn cast(self) -> $to {
                self as $to
            }
        }
    };
}

impl_cast!(u8, i8);
impl_cast!(u16, i16);
impl_cast!(u32, i32);
impl_cast!(u64, i64);
impl_cast!(u128, i128);
impl_cast!(i8, u8);
impl_cast!(i16, u16);
impl_cast!(i32, u32);
impl_cast!(i64, u64);
impl_cast!(i128, u128);
