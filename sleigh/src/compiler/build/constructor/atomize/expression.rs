//! This module implements the atomization of PCodeExpressions.
//! The entry function is [SleighCompiler::atomize_expression].

mod binary;

use crate::compiler::ast::common::SASTLiteral;
use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::{
    SASTPCodeExpression,
    SASTPCodeUnaryOp,
};
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::report::{
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
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};
use std::cmp::Ordering;
use std::fmt::Display;
use std::ops::{
    Neg,
    Not,
};
use tracing::trace;

impl SleighCompiler {
    /// Atomizes the expression into multiple MOIRInstructions.
    /// The function returns a [MOIRLocal] that contains the result of the expression.
    /// If an out local was specified, it may be used to directly store the result.
    /// In this case, the returned [MOIRLocal] will be the same as the provided out.
    /// The result may not be stored in out, if it is for example a const value to give
    /// other operations the opportunity to further fold the constants.
    /// If not out local was specified, then this function returns a new [MOIRLocal].
    /// If the expression contains only constants, then this can be optimized down to
    /// a single const [MOIRLocal::Const]. Otherwise, [MOIRLocal] will be [MOIRLocal::Local].
    pub fn atomize_expression(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (expression, _) = expression.into_tuple();
        trace!("Atomizing expression {} with out {:?}", expression, out);
        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);

        use SASTPCodeExpression::*;
        match expression {
            Invocation(invocation) => self.atomize_invocation(scope, instructions, out, invocation),
            BinaryOperation { lhs, op, rhs } => self.atomize_binary_operation(scope, instructions, out, *lhs, op, *rhs),
            UnaryOperation { op, operand } => self.atomize_unary_operation(scope, instructions, out, op, *operand),
            Dereference { .. } => self.atomize_dereference(instructions),
            Subpiece(subpiece) => self.atomize_subpiece(scope, out_size, subpiece),
            Bitrange(bitrange) => self.atomize_bitrange(scope, instructions, out, bitrange),
            Identifier(ident) => self.atomize_identifier(scope, out_size, ident.transmute()),
            Load(deref) => self.atomize_load(scope, instructions, out, deref),
            AddressOf(address_of) => self.atomize_address_of(scope, instructions, out, address_of),
            Literal(literal) => self.atomize_literal(out_size, literal),
            Error(_) => {
                panic!("An error in the AST should have lead to the compiler quitting earlier on.")
            }
        }
    }
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

    pub fn atomize_dereference(
        &mut self,
        _instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRLocal> {
        todo!()
    }

    /// The subpiece operation throws away the lower N bits.
    /// The resulting size of the output is thus (size - N).
    /// Since we simply transform a local with this atomization,
    /// we only need a size constraint and no out node and also
    /// always return a local.
    pub fn atomize_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        size_constraint: MOIRMaybeSize,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (subpiece, _) = subpiece.into_tuple();
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

    pub fn atomize_bitrange(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        bitrange: Spanned<SASTPCodeBitRange>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (bitrange, _) = bitrange.into_tuple();
        let value = scope.resolve_ident(&self.spec_builder, bitrange.ident.transmute())?;
        let shift_amount = MOIRLocal::from(bitrange.lsb.into_value());
        let bits = bitrange.bits.into_value();
        // The size of the output is the smallest size in bytes that still fits the output.
        let size = (bits + 7) / 8;

        // Allocate an out local if none was provided. Otherwise, we need to make sure,
        // that the size of out is correct.
        let out = if let Some(out) = out {
            scope.constrain_local_size(&mut MOIRLocal::from(out), size)?;
            out
        } else {
            scope.allocate_temporary(size)
        };

        // Bitrange is an emulated instruction. We first shift the bits right until we have the lsb at position 0
        // We do not want to modify the original value, so we need to create a copy here.
        let tmp = scope.allocate_temporary(scope.size_of_local(value));
        instructions.push(moir_instruction!(tmp = value >> shift_amount));

        // We can now reduce the size of the slice, discarding the remaining bits.
        let tmp = scope.truncate_local(tmp, size)?;
        instructions.push(moir_instruction!(out = tmp));

        // And then mask out the bits that remain which we don't need.
        // Truncate the mask to correct size before storing it in a local
        let mask = if size < 16 {
            // The mask constant can be calculated internally. If it is too large we have to calculate in pcode.
            MOIRLocal::from(MOIRConst::from((1u128 << bits) - 1).truncate(size)?)
        } else {
            // The mask is too large, calculate in pcode.
            // TODO: We might be able to do this as constant calculation in the future.
            let mask = scope.allocate_temporary(size);
            instructions.push(moir_instruction!(mask = { MOIRConst::new_with_size(1, size)? } << bits));
            instructions.push(moir_instruction!(mask = mask - { MOIRConst::new_with_size(1, size)? }));
            mask.into()
        };
        instructions.push(moir_instruction!(out = out & mask));
        Ok(out.into())
    }

    pub fn atomize_load(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing load expression");
        let (deref, _) = deref.into_tuple();
        let value_size = deref
            .size
            .map(Spanned::into_value)
            .map(MOIRMaybeSize::Determined)
            .unwrap_or(MOIRMaybeSize::Undetermined);
        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);

        // If out is not specified we allocate our own out variable to store the result in.
        let out = out.unwrap_or_else(|| scope.allocate_temporary(out_size.or(value_size)));
        let space = self
            .spec_builder
            .resolve_space(deref.space.map(Spanned::transmute))?;
        let addr = self.atomize_expression(scope, instructions, None, *deref.expression)?;
        if let MOIRMaybeSize::Determined(out_size) = out_size {
            if let MOIRMaybeSize::Determined(value_size) = value_size {
                match out_size.cmp(&value_size) {
                    Ordering::Less => {
                        todo!("Generate error for out too small for loaded value")
                    }
                    Ordering::Equal => {
                        // If the size of out is equal to the size of the load, we can just perform the load.
                        instructions.push(moir_instruction!(out = *[space]addr));
                    }
                    Ordering::Greater => {
                        // If the size of out is larger than the loaded value, we perform a zero extend.
                        // Insert the operation here. The jit might optimize the zero extend + load into one op.
                        // Allocate tmp for loading the value into a correctly sized varnode.
                        let tmp = scope.allocate_temporary(value_size);
                        // Load the value
                        instructions.push(moir_instruction!(tmp = *[space]addr));
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
    pub fn atomize_address_of(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        address_of: Spanned<SASTPCodeAddressOf>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (address_of, _) = address_of.into_tuple();
        let size = address_of
            .size
            .map(|s| MOIRMaybeSize::Determined(s.into_value()))
            .unwrap_or(MOIRMaybeSize::Undetermined);
        if let MOIRMaybeSize::Determined(size_constraint) = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined)
        {
            // If out has a size and the address_of operator also specifies
            // a size, they have to match.
            if let MOIRMaybeSize::Determined(size) = size {
                if size != size_constraint {
                    todo!("Generate error report for mismatched sizes")
                }
            }
        }
        // The size of rhs is not relevant, so we leave it up to the atomization to allocate if necessary.
        let rhs = self.atomize_expression(scope, instructions, None, *address_of.expression)?;
        let lhs = out.unwrap_or_else(|| scope.allocate_temporary(size));
        instructions.push(moir_instruction!(lhs = &rhs));
        Ok(lhs.into())
    }

    /// Atomizing an identifier always returns a local since we don't want to simply copy this into an out local.
    /// Hence, we also just need a size constraint if applicable.
    pub fn atomize_identifier(
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
                scope.resolve_ident(&self.spec_builder, ident)?
            }
        };
        Ok(local.into())
    }

    /// Atomizing a literal always returns a local since we don't want to simply copy this into an out local.
    /// Hence, we also just need a size constraint if applicable.
    pub fn atomize_literal(
        &mut self,
        size_constraint: MOIRMaybeSize,
        literal: Spanned<SASTLiteral>,
    ) -> SleighCompilerResult<MOIRLocal> {
        Ok(MOIRConst::new_with_size(literal.into_value(), size_constraint)?.into())
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

fn into_bool<T>(val: T) -> bool
where
    T: Into<u128>,
{
    !matches!(val.into(), 0u128)
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
