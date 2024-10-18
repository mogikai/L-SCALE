use crate::compiler::ast::common::{
    SASTIdent,
    SASTSize,
};
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::Spanned;
use crate::compiler::error::{CannotSliceLhs, SleighCompilerError};
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::ProcessorSpecificationBuilder;
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;

impl SleighCompiler {
    pub fn atomize_assignment_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> SleighCompilerResult<()> {
        // We are dealing with a subpiece assignment. This is similar to a normal assignment.
        // However, we create a new slice and already have the required size, since it is
        // specified in the subpiece operation.
        let (subpiece, _) = subpiece.into_tuple();
        match subpiece.target.into_value() {
            SASTPCodeSubpieceTarget::Ident(ident) => {
                self.handle_ident_target(scope, instructions, local, expression, subpiece.selector, ident)?;
            }
            SASTPCodeSubpieceTarget::Literal(_) => {
                todo!("Generate error report for unallowed literals in target expression")
            }
        }
        Ok(())
    }

    fn handle_ident_target(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        selector: Spanned<SASTPCodeSubpieceSelector>,
        ident: Spanned<SASTIdent>,
    ) -> Result<(), SleighCompilerError> {
        let size = match selector.into_value() {
            SASTPCodeSubpieceSelector::Take(size) => size,
            SASTPCodeSubpieceSelector::Discard(_) => {
                todo!("Currently unimplemented")
            }
        };
        let lhs = resolve_lhs(&self.spec_builder, scope, local.clone(), ident.clone(), size)?;
        ensure_valid_subpiece_size(scope, lhs, size)?;
        let lhs = truncate_lhs(&self.spec_builder, scope, local, ident, lhs, size)?;
        // The variable is not declared yet. First resolve the rhs and try to determine a size.
        let rhs = self.atomize_expression(scope, instructions, Some(lhs), expression)?;
        if MOIRLocal::from(lhs) != rhs {
            // The atomization could not directly assign to lhs, so we need to emit a copy.
            instructions.push(moir_instruction!(lhs = rhs));
        }
        Ok(())
    }
}

fn ensure_valid_subpiece_size(
    scope: &MOIRScope,
    lhs: MOIRLocalId,
    size: SASTSize,
) -> SleighCompilerResult<()> {
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
    Ok(())
}

fn resolve_lhs(
    spec_builder: &ProcessorSpecificationBuilder,
    scope: &mut MOIRScope,
    local: Spanned<bool>,
    ident: Spanned<SASTIdent>,
    size: SASTSize,
) -> SleighCompilerResult<MOIRLocalId> {
    if local.into_value() {
        // We encountered a local definition. Define here and error out if it already exists.
        scope.allocate_named_temporary(ident.transmute(), size.into())
    } else {
        scope.resolve_ident(spec_builder, ident.transmute())
    }
}

fn truncate_lhs(
    spec_builder: &ProcessorSpecificationBuilder,
    scope: &mut MOIRScope,
    local: Spanned<bool>,
    ident: Spanned<SASTIdent>,
    lhs: MOIRLocalId,
    size: SASTSize,
) -> Result<MOIRLocalId, SleighCompilerError> {
    if *local {
        // If we defined a new local, then there is no need to truncate.
        Ok(lhs)
    } else {
        scope.truncate_local(lhs, size).map_err(|e| {
            let (ident, ident_span) = ident.into_tuple();
            let str = spec_builder.resolve_string_id(ident.into());
            SleighCompilerError::CannotSliceLhs(CannotSliceLhs {
                err: Box::new(e),
                ident_str: str.to_string(),
                ident: ident_span,
                expected_size: size,
            })
        })
    }
}
