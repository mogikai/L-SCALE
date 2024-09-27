//! This module implements the atomization of PCodeExpressions.
//! The entry function is [SleighCompiler::atomize_expression].

mod address_of;
mod binary;
mod bitrange;
mod identifier;
mod load;
mod subpiece;
mod unary;

use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTLiteral;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::build::trace_build;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};

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
        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);

        use SASTPCodeExpression::*;
        match expression {
            Invocation(invocation) => self
                .atomize_invocation(scope, instructions, out, invocation)?
                .ok_or_else(|| todo!("Generate error report for invocation without output")),
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

    pub fn atomize_dereference(
        &mut self,
        _instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<MOIRLocal> {
        todo!()
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
