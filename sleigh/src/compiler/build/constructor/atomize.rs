mod expression;

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    MOIRScope,
    MOIRTemporarySize,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRSlice,
    MOIRSliceEnd,
    MOIRValue,
    MOIRVarNode,
    MOIRVarNodeId,
};
use crate::spec::MOIRSymbol;
use tracing::trace;

impl SleighCompiler {
    pub(super) fn atomize_pcode(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        pcode: Spanned<SASTPCode>,
    ) -> SleighCompilerResult<Option<MOIRValue>> {
        // TODO: Create table of locals? Lookup global identifiers?
        let (pcode, _) = pcode.into_tuple();
        let mut export = None;
        for ((label, statement), _) in pcode.statements.into_iter().map(Spanned::into_tuple) {
            if export.is_some() {
                todo!("Generate error report for instruction after export statement")
            }
            let (statement, _) = statement.into_tuple();
            match statement {
                SASTPCodeStatement::Assignment {
                    local,
                    target,
                    expression,
                } => self.atomize_assignment(scope, instructions, local, target, expression)?,
                SASTPCodeStatement::Invocation(invocation) => {
                    // We ignore the output, since top level invocations do not output anything.
                    // TODO: Maybe it makes sense to update the pcodeop definition here to "no output"?
                    self.atomize_invocation(scope, instructions, invocation)?;
                }
                SASTPCodeStatement::Build(_) => {}
                SASTPCodeStatement::Export(expression) => {
                    let span = expression.span().clone();
                    export = Some(Spanned::new(self.atomize_export(instructions, expression)?, span));
                }
                SASTPCodeStatement::Delayslot(_) => {}
                SASTPCodeStatement::Declaration(ident, size) => {
                    self.atomize_declaration(scope, ident.transmute(), size.map(Spanned::transmute))?
                }
                SASTPCodeStatement::Branch(_) => {}
                SASTPCodeStatement::ConditionalBranch { .. } => {}
                SASTPCodeStatement::BranchCall(_) => {}
                SASTPCodeStatement::BranchReturn(_) => {}
                SASTPCodeStatement::NOP => {}
                SASTPCodeStatement::Error(_) => {}
            }
        }
        trace!("Instructions: {instructions:?}");
        Ok(export.map(|export| export.into_value()))
    }

    /// Atomizes the expression into multiple MOIRInstructions.
    /// The function returns a [MOIRValue] that contains the result of the expression.
    /// If the expression contains only constants, then this can be optimized down to
    /// a single const [MOIRValue]. Otherwise, [MOIRValue] will be [MOIRValue::VarNode].
    fn atomize_expression(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRValue> {
        let (expression, _) = expression.into_tuple();
        trace!("Atomizing expression {}", expression);
        match expression {
            SASTPCodeExpression::Invocation(invocation) => self.atomize_invocation(scope, instructions, invocation),
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                self.atomize_binary_operation(scope, instructions, *lhs, op, *rhs)
            }
            SASTPCodeExpression::UnaryOperation { op, operand } => {
                self.atomize_unary_operation(scope, instructions, op, *operand)
            }
            SASTPCodeExpression::Dereference { .. } => self.atomize_dereference(instructions),
            SASTPCodeExpression::Subpiece(_) => self.atomize_subpiece(instructions),
            SASTPCodeExpression::Bitrange(_) => self.atomize_bitrange(instructions),
            SASTPCodeExpression::Identifier(_) => self.atomize_identifier(instructions),
            SASTPCodeExpression::Load(_) => self.atomize_load(instructions),
            SASTPCodeExpression::AddressOf(_) => self.atomize_address_of(instructions),
            SASTPCodeExpression::Literal(literal) => self.atomize_literal(instructions, literal),
            SASTPCodeExpression::Error(_) => {
                panic!("An error in the AST should have lead to the compiler quitting earlier on.")
            }
        }
    }

    fn atomize_assignment(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        target: Spanned<SASTPCodeTargetExpression>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult {
        let (target, _) = target.into_tuple();
        let value = match target {
            SASTPCodeTargetExpression::Variable(var) => {
                if local.into_value() {
                    scope.allocate_named_temporary(var.clone().transmute(), MOIRTemporarySize::Undetermined)?;
                }
                scope.resolve_value(&self.spec_builder, var.transmute())?
            }
            SASTPCodeTargetExpression::Store(deref) => {
                if *local {
                    todo!("Generate error report for local keyword on store assignment")
                }
                MOIRValue::ValueSlice(MOIRSlice::varnode(MOIRVarNodeId::from(0), 0, 0))
            }
            SASTPCodeTargetExpression::BitRange(bitrange) => {
                let (bitrange, _) = bitrange.into_tuple();
                if local.into_value() {
                    scope.allocate_named_temporary(
                        bitrange.ident.clone().transmute(),
                        MOIRTemporarySize::Undetermined,
                    )?;
                }
                scope.resolve_value(&self.spec_builder, bitrange.ident.transmute())?
            }
            SASTPCodeTargetExpression::Subpiece(subpiece) => {
                let (subpiece, _) = subpiece.into_tuple();
                match subpiece.target.into_value() {
                    SASTPCodeSubpieceTarget::Ident(ident) => {
                        let size = subpiece.size.into_value();
                        if local.into_value() {
                            scope.allocate_named_temporary(ident.clone().transmute(), size.into())?;
                        }
                        let value = scope.resolve_value(&self.spec_builder, ident.transmute())?;
                        if size
                            > value.size().ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error for undetermined size")
                        })?
                        {
                            todo!("Generate error for subpiece being larger than underlying slice.")
                        }
                        value
                    }
                    SASTPCodeSubpieceTarget::Literal(_) => {
                        todo!("Generate error report for unallowed literals in target expression")
                    }
                }
            }
        };
        trace!("Resolved target to {value:?}");
        // TODO: Check if target ident exists. If not, require the local attribute to create a new local variable.
        //       If the target is a store, require that the ident exists, and deny the local keyword, since it makes
        //       no sense in this context.
        // TODO: Atomize expression on the right hand side of the assignment.
        // TODO: Check if the sizes on the left hand side match the right hand side.
        let rhs = self.atomize_expression(scope, instructions, expression)?;
        trace!("Assignment: {:?} = {:?}", value, rhs);
        Ok(())
    }

    fn atomize_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult<MOIRValue> {
        trace!("Atomizing invocation with {} arguments.", invocation.value().arguments.len());
        let output = scope.allocate_temporary();
        instructions.push(MOIRInstruction {
            input_a: MOIRValue::Const(MOIRConst::Const8(0)),
            input_b: Some(MOIRValue::Const(MOIRConst::Const8(0))),
            output,
        });
        Ok(output.into())
    }

    fn atomize_export(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO: Handling dynamic references (e.g. export *:4 tmp) will require keeping track of the
        //       dynamic references somewhere. We need to decide how to do this.
        Ok(MOIRValue::ValueSlice(MOIRSlice::varnode(MOIRVarNodeId::from(0), 0, 8)))
    }

    fn atomize_declaration(
        &mut self,
        scope: &mut MOIRScope,
        ident: Spanned<StringId>,
        size: Option<Spanned<MOIRSize>>,
    ) -> SleighCompilerResult {
        scope.allocate_named_temporary(
            ident,
            size.map_or(MOIRTemporarySize::Undetermined, |size| size.into_value().into()),
        )
    }
}
