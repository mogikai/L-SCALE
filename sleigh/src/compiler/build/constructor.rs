use crate::compiler::ast::constructor::bit_pattern_section::SASTBitPatternConstraint;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::pcode::statement::SASTPCodeStatement;
use crate::compiler::ast::pcode::target_expression::SASTPCodeTargetExpression;
use crate::compiler::ast::pcode::SASTPCode;
use crate::compiler::ast::Spanned;
use crate::compiler::{
    CompilerContext,
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    MOIRConst,
    MOIRInstruction,
    MOIRValue,
};
use crate::spec::moir::varnode::MOIRVarNodeId;
use tracing::trace;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if it does not exist.
    /// Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        let (constructor, span) = constructor.into_tuple();
        let table_name = constructor
            .table
            .value()
            .map(|sid| self.spec_builder.string(sid))
            .flatten()
            .unwrap_or("instruction");
        trace!("Defining constructor in table {:?}", table_name);
        let table = match constructor.table.lift_option() {
            None => self.spec_builder.modify_root_table()?,
            Some(id) => self.spec_builder.modify_table(id)?,
        };
        self.validate_constraint(constructor.bit_pattern_section.into_value().constraint)?;
        // TODO: we need to walk the constraint and find out the tokens that are used.
        //       we also need to make sure that there is only one token per section
        //       delimited by a semicolon.

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        self.atomize_pcode(constructor.semantic_section.into_value().0)?;
        Ok(())
    }

    fn validate_constraint(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult {
        let (constraint, span) = constraint.into_tuple();
        match constraint {
            SASTBitPatternConstraint::Ident(_) => {}
            SASTBitPatternConstraint::BinaryOp(_, _, _) => {}
            SASTBitPatternConstraint::UnaryOp(_, _) => {}
            SASTBitPatternConstraint::AdvancedConstraint(_, _, _) => {}
            SASTBitPatternConstraint::Invalid => {}
        };
        Ok(())
    }

    fn atomize_pcode(
        &mut self,
        pcode: Spanned<SASTPCode>,
    ) -> SleighCompilerResult<Vec<MOIRInstruction>> {
        // TODO: Create table of locals? Lookup global identifiers?
        let (pcode, _) = pcode.into_tuple();
        let mut instructions = vec![];
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
                } => self.atomize_assignment(&mut instructions, local, target, expression)?,
                SASTPCodeStatement::Invocation(invocation) => self.atomize_invocation(&mut instructions, invocation)?,
                SASTPCodeStatement::Build(_) => {}
                SASTPCodeStatement::Export(expression) => {
                    export = Some(expression.span().clone());
                    self.atomize_export(&mut instructions, expression)?;
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
        Ok(instructions)
    }

    fn atomize_assignment(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        target: Spanned<SASTPCodeTargetExpression>,
        expression: Spanned<SASTPCodeExpression>,
    ) -> SleighCompilerResult {
        if *local && matches!(*target, SASTPCodeTargetExpression::Store(_)) {
            todo!("Generate error report for local keyword on store assignment")
        }
        // TODO: Check if target ident exists. If not, require the local attribute to create a new local variable.
        //       If the target is a store, require that the ident exists, and deny the local keyword, since it makes
        //       no sense in this context.
        // TODO: Atomize expression on the right hand side of the assignment.
        // TODO: Check if the sizes on the left hand side match the right hand side.
        Ok(())
    }

    fn atomize_invocation(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult {
        instructions.push(MOIRInstruction {
            input_a: MOIRValue::Const(MOIRConst::Const8(0)),
            input_b: MOIRValue::Const(MOIRConst::Const8(0)),
            output: MOIRVarNodeId::from(0),
        });
        Ok(())
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
