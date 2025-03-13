use crate::compiler::error::{
    CannotTruncateLhs,
    InternalError,
};
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::builder::ProcessorSpecificationBuilder;
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::common::{
    SASTIdent,
    SASTSize,
};
use mogi_sleigh_parser::ast::pcode::expression::SASTPCodeExpression;
use mogi_sleigh_parser::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};

impl SleighCompiler {
    pub fn atomize_assignment_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> InternalCompilerResult<()> {
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
    ) -> Result<(), InternalError> {
        let size = match selector.into_value() {
            SASTPCodeSubpieceSelector::Take(size) => size,
            SASTPCodeSubpieceSelector::Discard(_) => {
                todo!("Currently unimplemented")
            }
        }
        .try_into()
        .expect("The size must fit into u8");
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
    size: MOIRSize,
) -> InternalCompilerResult<()> {
    // If the subpiece tries to create a slice that is
    // larger than the original, we generate an error.
    if size
        > scope.size_of_local(lhs).ok_or_else(|| -> InternalError {
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
    size: MOIRSize,
) -> InternalCompilerResult<MOIRLocalId> {
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
    size: MOIRSize,
) -> InternalCompilerResult<MOIRLocalId> {
    if *local {
        // If we defined a new local, then there is no need to truncate.
        Ok(lhs)
    } else {
        scope.truncate_local(lhs, size).map_err(|err| {
            let (ident, ident_span) = ident.into_tuple();
            let str = spec_builder.resolve_string_id(ident.into());
            let lhs_err = CannotTruncateLhs {
                ident_str: str.to_string(),
                expected_size: size,
            };
            InternalError::with_span(lhs_err.into(), "ident", ident_span).cause(err)
        })
    }
}
