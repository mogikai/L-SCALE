mod expression;

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::MOIRInstruction;
use crate::spec::moir::scope::MOIRScope;
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRSlice,
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
    ) -> SleighCompilerResult<Option<()>> {
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
                    self.atomize_invocation(instructions, invocation)?;
                }
                SASTPCodeStatement::Build(_) => {}
                SASTPCodeStatement::Export(expression) => {
                    export = Some(expression.span().clone());
                    self.atomize_export(instructions, expression)?;
                }
                SASTPCodeStatement::Delayslot(_) => {}
                SASTPCodeStatement::Declaration(_, _) => {}
                SASTPCodeStatement::Branch(_) => {}
                SASTPCodeStatement::ConditionalBranch { .. } => {}
                SASTPCodeStatement::BranchCall(_) => {}
                SASTPCodeStatement::BranchReturn(_) => {}
                SASTPCodeStatement::NOP => {}
                SASTPCodeStatement::Error(_) => {}
            }
        }
        trace!("Instructions: {instructions:?}");
        Ok(export.map(|_| ()))
    }

    /// Atomizes the expression into multiple MOIRInstructions.
    /// The function returns a [MOIRValue] that contains the result of the expression.
    /// If the expression contains only constants, then this can be optimized down to
    /// a single const [MOIRValue]. Otherwise, [MOIRValue] will be [MOIRValue::VarNode].
    fn atomize_expression(
        &mut self,
        out: MOIRVarNodeId,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRValue> {
        let (expression, _) = expression.into_tuple();
        trace!("Atomizing expression {}", expression);
        match expression {
            SASTPCodeExpression::Invocation(invocation) => self.atomize_invocation(instructions, invocation),
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                self.atomize_binary_operation(out, instructions, *lhs, op, *rhs)
            }
            SASTPCodeExpression::UnaryOperation { .. } => self.atomize_unary_operation(instructions),
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
        match target {
            SASTPCodeTargetExpression::Variable(var) => {
                let result = scope.resolve_value(&self.spec_builder, var.transmute());
                self.report_error_or_unwrap(result)?;
            }
            SASTPCodeTargetExpression::Store(deref) => {
                if *local {
                    todo!("Generate error report for local keyword on store assignment")
                }
            }
            SASTPCodeTargetExpression::BitRange(bitrange) => {}
            SASTPCodeTargetExpression::Subpiece(subpiece) => {}
        }
        // TODO: Check if target ident exists. If not, require the local attribute to create a new local variable.
        //       If the target is a store, require that the ident exists, and deny the local keyword, since it makes
        //       no sense in this context.
        // TODO: Atomize expression on the right hand side of the assignment.
        // TODO: Check if the sizes on the left hand side match the right hand side.
        let out = MOIRVarNodeId::from(0);
        self.atomize_expression(out, instructions, expression)?;
        Ok(())
    }

    fn atomize_invocation(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult<MOIRValue> {
        trace!("Atomizing invocation with {} arguments.", invocation.value().arguments.len());
        let output = MOIRVarNodeId::from(0);
        instructions.push(MOIRInstruction {
            input_a: MOIRValue::Const(MOIRConst::Const8(0)),
            input_b: MOIRValue::Const(MOIRConst::Const8(0)),
            output,
        });
        Ok(MOIRValue::ValueSlice(MOIRSlice::varnode(output, 0, 0)))
    }

    fn atomize_export(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult {
        // TODO: Handling dynamic references (e.g. export *:4 tmp) will require keeping track of the
        //       dynamic references somewhere. We need to decide how to do this.
        Ok(())
    }
}
