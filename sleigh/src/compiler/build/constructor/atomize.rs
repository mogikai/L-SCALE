mod assignment;
mod expression;
mod size_inference;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::{
    SASTPCodeBranchTarget,
    SASTPCodeStatement,
};
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
use crate::spec::builder::{
    MOIRSymbolId,
    StringId,
};
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
    MOIROpCategory,
    MOIROperation,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRSlice,
    MOIRVarNode,
    VarNodeId,
};
use tracing::{
    debug,
    trace,
};

use std::fmt::Write;

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
                    self.atomize_invocation(scope, instructions, None, invocation)?;
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
                SASTPCodeStatement::Branch(branch) => self.atomize_branch(scope, instructions, branch)?,
                SASTPCodeStatement::ConditionalBranch { .. } => {}
                SASTPCodeStatement::BranchCall(_) => {}
                SASTPCodeStatement::BranchReturn(_) => {}
                SASTPCodeStatement::NOP => {}
                SASTPCodeStatement::Error(_) => {
                    panic!("If there is an error in the AST, then this function should never have been invoked");
                }
            }
        }
        self.infer_sizes(scope, instructions)?;
        debug_instructions(instructions);
        Ok(export.map(|export| match export.into_value() {
            MOIRLocal::Local(local) => MOIRExport::Slice(*scope.resolve_local(local).expect("TODO")),
            MOIRLocal::Const(val) => MOIRExport::Const(val),
        }))
    }

    /// Atomizes the expression into multiple MOIRInstructions.
    /// The function returns a [MOIRLocal] that contains the result of the expression.
    /// If an out local was specified, it may be used to directly store the result.
    /// In this case, the returned [MOIRLocal] will be the same as the provided out.
    /// The result may not be stored in out, if it is for example a const value to give
    /// other operations the opportunity to further fold the constants.
    /// If not out local was specified, then this function returns a new [MOIRLocal].
    /// If the expression contains only constants, then this can be optimized down to
    /// a single const [MOIRLocal::Const]. Otherwise, [MOIRLocal] will be [MOIRLocal::Local].
    fn atomize_expression(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (expression, _) = expression.into_tuple();
        trace!("Atomizing expression {}", expression);
        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);
        match expression {
            SASTPCodeExpression::Invocation(invocation) => {
                self.atomize_invocation(scope, instructions, out, invocation)
            }
            SASTPCodeExpression::BinaryOperation { lhs, op, rhs } => {
                self.atomize_binary_operation(scope, instructions, out, *lhs, op, *rhs)
            }
            SASTPCodeExpression::UnaryOperation { op, operand } => {
                self.atomize_unary_operation(scope, instructions, out, op, *operand)
            }
            SASTPCodeExpression::Dereference { .. } => self.atomize_dereference(instructions),
            SASTPCodeExpression::Subpiece(subpiece) => self.atomize_subpiece(scope, instructions, out_size, subpiece),
            SASTPCodeExpression::Bitrange(bitrange) => self.atomize_bitrange(scope, instructions, bitrange),
            SASTPCodeExpression::Identifier(ident) => self.atomize_identifier(scope, out_size, ident.transmute()),
            SASTPCodeExpression::Load(deref) => self.atomize_load(scope, instructions, out, deref),
            SASTPCodeExpression::AddressOf(address_of) => self.atomize_address_of(scope, instructions, out, address_of),
            SASTPCodeExpression::Literal(literal) => self.atomize_literal(out_size, literal),
            SASTPCodeExpression::Error(_) => {
                panic!("An error in the AST should have lead to the compiler quitting earlier on.")
            }
        }
    }

    fn atomize_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing invocation with {} arguments.", invocation.value().arguments.len());
        let (invocation, _) = invocation.into_tuple();
        let output = out.unwrap_or_else(|| scope.allocate_temporary(MOIRMaybeSize::Undetermined));
        let mut args = invocation
            .arguments
            .into_iter()
            .map(|arg| self.atomize_expression(scope, instructions, None, arg));
        let input_a = args.next().transpose()?;
        let input_b = args.next().transpose()?;
        let args: Result<Vec<_>, _> = args.collect();
        instructions.push(MOIRInstruction {
            output: Some(output),
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
        self.atomize_expression(scope, instructions, None, expression)
    }

    fn atomize_declaration(
        &mut self,
        scope: &mut MOIRScope,
        ident: Spanned<StringId>,
        size: Option<Spanned<MOIRSize>>,
    ) -> SleighCompilerResult {
        scope
            .allocate_named_temporary(ident, size.map_or(MOIRMaybeSize::Undetermined, |size| size.into_value().into()))
            .map(|_| ())
    }

    fn atomize_branch(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        branch: Spanned<SASTPCodeBranchTarget>,
    ) -> SleighCompilerResult {
        let (branch, _) = branch.into_tuple();
        match branch {
            SASTPCodeBranchTarget::Label(label) => {
                let target = scope.resolve_ident(&self.spec_builder, label.transmute())?;
                instructions.push(moir_instruction!(goto<target>))
            }
            SASTPCodeBranchTarget::Absolute(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                instructions.push(moir_instruction!(goto[target]))
            }
            SASTPCodeBranchTarget::Relative(expr) => {
                let target = self.atomize_expression(scope, instructions, None, expr)?;
                instructions.push(moir_instruction!(goto target));
            }
            SASTPCodeBranchTarget::Invalid(_) => {
                panic!("An invalid branch target should have lead to the compiler quitting earlier.")
            }
        };
        Ok(())
    }
}

fn debug_instructions(instructions: &mut Vec<MOIRInstruction>) {
    let mut instructions_str = String::new();
    writeln!(&mut instructions_str, "The constructor pcode was atomized to the following instructions:").expect("");
    for instruction in instructions {
        writeln!(&mut instructions_str, "{}", instruction).expect("");
    }
    debug!("{instructions_str}");
}
