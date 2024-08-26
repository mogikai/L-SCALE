mod expression;

use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::report::{
    generate_cannot_slice_error,
    generate_const_eval_error,
};
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
    MOIROpCategory,
    MOIROperation,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRSlice,
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
    ) -> SleighCompilerResult<Option<MOIRExport>> {
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
                    self.atomize_invocation(scope, instructions, MOIRMaybeSize::Undetermined, invocation)?;
                }
                SASTPCodeStatement::Build(_) => {}
                SASTPCodeStatement::Export(expression) => {
                    let span = expression.span().clone();
                    export = Some(Spanned::new(self.atomize_export(scope, instructions, expression)?, span));
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
        self.infer_sizes(scope, instructions)?;
        trace!("Instructions: {instructions:?}");
        Ok(export.map(|export| match export.into_value() {
            MOIRLocal::Local(local) => MOIRExport::Slice(*scope.resolve_local(local).expect("TODO")),
            MOIRLocal::Const(val) => MOIRExport::Const(val),
        }))
    }

    fn infer_sizes(&mut self, scope: &mut MOIRScope, instructions: &mut Vec<MOIRInstruction>) -> SleighCompilerResult<()> {
        trace!("Doing size inference pass");
        for instruction in instructions.iter_mut() {
            trace!("{instruction:?}");
            let output_size = scope.size_of_local(instruction.output);
            let input_a_size = instruction
                .input_a
                .map(|v| v.size(scope))
                .unwrap_or(MOIRMaybeSize::Undetermined);
            let input_b_size = instruction
                .input_b
                .map(|v| v.size(scope))
                .unwrap_or(MOIRMaybeSize::Undetermined);
            trace!("{output_size:?} {input_a_size:?} {input_b_size:?}");
            match instruction.operation.op_category() {
                MOIROpCategory::Comparison => {}
                MOIROpCategory::Shift => {}
                MOIROpCategory::Regular => {
                    // For regular operations all sizes have to match.
                    // Check if any part of the operation has a determined size.
                    let size = output_size.or(input_a_size).or(input_b_size);
                    if let MOIRMaybeSize::Determined(size) = size {
                        // If we have a size, we can constrain all values to this size.
                        // Otherwise, we have to wait until the backwards pass.
                        scope.constrain_local_size(&mut MOIRLocal::from(instruction.output), size)?;
                        if let Some(local) = instruction.input_a.as_mut() {
                            scope.constrain_local_size(local, size)?
                        }
                        if let Some(local) = instruction.input_b.as_mut() {
                            scope.constrain_local_size(local, size)?
                        }
                    }
                }
                MOIROpCategory::Special => {
                    match instruction.operation {
                        MOIROperation::Load(_) => {
                            // For load we only need to make sure the address is the correct size.
                            // TODO: Where do we get the correct address size from?
                            // TODO: This probably needs to match with inst_next?
                        }
                        MOIROperation::ZeroExtend => {
                            // We cannot do any size inference, but we can make sure that the
                            // sizes are valid, i.e. that output_size > input_size
                            if let MOIRMaybeSize::Determined(output_size) = output_size {
                                if let MOIRMaybeSize::Determined(input_a_size) = input_a_size {
                                    if output_size <= input_a_size {
                                        todo!("Generate error report for invalid sizes for zext")
                                    }
                                }
                            }
                        }
                        _ => {
                            // There is nothing to do in this case.
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Atomizes the expression into multiple MOIRInstructions.
    /// The function returns a [MOIRValue] that contains the result of the expression.
    /// If the expression contains only constants, then this can be optimized down to
    /// a single const [MOIRValue]. Otherwise, [MOIRValue] will be [MOIRValue::VarNode].
    fn atomize_expression(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (expression, _) = expression.into_tuple();
        trace!("Atomizing expression {}", expression);
        match expression {
            SASTPCodeExpression::Invocation(invocation) => {
                self.atomize_invocation(scope, instructions, size_constraint, invocation)
            }
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                self.atomize_binary_operation(scope, instructions, size_constraint, *lhs, op, *rhs)
            }
            SASTPCodeExpression::UnaryOperation { op, operand } => {
                self.atomize_unary_operation(scope, instructions, size_constraint, op, *operand)
            }
            SASTPCodeExpression::Dereference { .. } => self.atomize_dereference(instructions),
            SASTPCodeExpression::Subpiece(subpiece) => {
                self.atomize_subpiece(scope, instructions, size_constraint, subpiece)
            }
            SASTPCodeExpression::Bitrange(bitrange) => self.atomize_bitrange(scope, instructions, bitrange),
            SASTPCodeExpression::Identifier(ident) => {
                self.atomize_identifier(scope, size_constraint, ident.transmute())
            }
            SASTPCodeExpression::Load(deref) => self.atomize_load(scope, instructions, size_constraint, deref),
            SASTPCodeExpression::AddressOf(address_of) => {
                self.atomize_address_of(scope, instructions, size_constraint, address_of)
            }
            SASTPCodeExpression::Literal(literal) => self.atomize_literal(size_constraint, literal),
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
                let (lhs, rhs) = if local.into_value() {
                    // The variable is not declared yet. First resolve the rhs and try to determine a size.
                    let rhs = self.atomize_expression(scope, instructions, MOIRMaybeSize::Undetermined, expression)?;
                    scope.allocate_named_temporary(var.clone().transmute(), rhs.size(scope))?;
                    let lhs = scope.resolve_ident(&self.spec_builder, var.transmute())?;
                    (lhs, rhs)
                } else {
                    // The variable is already declared and maybe has a size. pass the size as hint.
                    let lhs = scope.resolve_ident(&self.spec_builder, var.transmute())?;
                    let rhs = self.atomize_expression(scope, instructions, scope.size_of_local(lhs), expression)?;
                    (lhs, rhs)
                };
                instructions.push(moir_instruction!(lhs = rhs));
                Ok(())
            }
            SASTPCodeTargetExpression::Store(deref) => {
                if *local {
                    todo!("Generate error report for local keyword on store assignment")
                }
                Ok(())
            }
            SASTPCodeTargetExpression::BitRange(bitrange) => {
                let (bitrange, _) = bitrange.into_tuple();
                if local.into_value() {
                    scope.allocate_named_temporary(bitrange.ident.clone().transmute(), MOIRMaybeSize::Undetermined)?;
                }
                scope.resolve_ident(&self.spec_builder, bitrange.ident.transmute())?;
                Ok(())
            }
            SASTPCodeTargetExpression::Subpiece(subpiece) => {
                // We are dealing with a subpiece assignment. This is similar to a normal assignment.
                // However, we create a new slice and already have the required size, since it is
                // specified in the subpiece operation.
                let (subpiece, _) = subpiece.into_tuple();
                match subpiece.target.into_value() {
                    SASTPCodeSubpieceTarget::Ident(ident) => {
                        let size = match subpiece.selector.into_value() {
                            SASTPCodeSubpieceSelector::Take(size) => size,
                            SASTPCodeSubpieceSelector::Discard(_) => {
                                todo!("Currently unimplemented")
                            }
                        };
                        if local.into_value() {
                            // We encountered a local definition. Define here and error out if it already exists.
                            scope.allocate_named_temporary(ident.clone().transmute(), size.into())?;
                        }
                        // We can now resolve the ident.
                        let lhs = scope.resolve_ident(&self.spec_builder, ident.clone().transmute())?;
                        // If the subpiece tries to create a slice that is
                        // larger than the original, we generate an error.
                        if size
                            > scope
                                .size_of_local(lhs)
                                .ok_or_else(|| -> SleighCompilerError {
                                    // We cannot slice a value that has an undetermined size
                                    todo!("Generate error for undetermined size")
                                })?
                        {
                            todo!("Generate error for subpiece being larger than underlying slice.")
                        }
                        let lhs = scope.truncate_local(lhs, size).map_err(|_| {
                            let (ident, ident_span) = ident.into_tuple();
                            let str = self.spec_builder.resolve_string_id(ident.into());
                            generate_cannot_slice_error(str, ident_span, size)
                        })?;
                        // The variable is not declared yet. First resolve the rhs and try to determine a size.
                        let rhs = self.atomize_expression(scope, instructions, scope.size_of_local(lhs), expression)?;
                        instructions.push(moir_instruction!(lhs = rhs));
                        Ok(())
                    }
                    SASTPCodeSubpieceTarget::Literal(_) => {
                        todo!("Generate error report for unallowed literals in target expression")
                    }
                }
            }
        }
    }

    fn atomize_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        size_constraint: MOIRMaybeSize,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing invocation with {} arguments.", invocation.value().arguments.len());
        let (invocation, _) = invocation.into_tuple();
        let output = scope.allocate_temporary(size_constraint);
        let mut args = invocation
            .arguments
            .into_iter()
            .map(|arg| self.atomize_expression(scope, instructions, MOIRMaybeSize::Undetermined, arg));
        let input_a = args.next().transpose()?;
        let input_b = args.next().transpose()?;
        let args: Result<Vec<_>, _> = args.collect();
        instructions.push(MOIRInstruction {
            output,
            input_a,
            input_b,
            operation: MOIROperation::CustomOp(args?),
        });
        Ok(output.into())
    }

    fn atomize_export(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        // TODO: Handling dynamic references (e.g. export *:4 tmp) will require keeping track of the
        //       dynamic references somewhere. We need to decide how to do this.
        self.atomize_expression(scope, instructions, MOIRMaybeSize::Undetermined, expression)
    }

    fn atomize_declaration(
        &mut self,
        scope: &mut MOIRScope,
        ident: Spanned<StringId>,
        size: Option<Spanned<MOIRSize>>,
    ) -> SleighCompilerResult {
        scope.allocate_named_temporary(ident, size.map_or(MOIRMaybeSize::Undetermined, |size| size.into_value().into()))
    }
}
