use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::Spanned;
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
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;

impl SleighCompiler {
    pub fn atomize_assignment_variable(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        var: Spanned<SASTIdent>,
    ) -> SleighCompilerResult<()> {
        let lhs = resolve_lhs(&self.spec_builder, scope, local, var)?;
        // We directly pass the out varnode to the atomize functions to avoid a separate copy operation,
        // since the expressions can directly store their result into this out variable.
        let rhs = self.atomize_expression(scope, instructions, Some(lhs), expression)?;
        if MOIRLocal::from(lhs) != rhs {
            // The atomization could not store into lhs so we need to generate a copy instruction.
            // This can for example happen, if rhs can be folded into a const, in which case
            // we simply copy the const into the lhs here.
            // We can additionally constrain the lhs size to the rhs size if it is determined.
            if scope.size_of_local(lhs).is_undetermined() {
                if let MOIRMaybeSize::Determined(size) = rhs.size(scope) {
                    scope.constrain_local_id_size(lhs, size)?;
                }
            }
            instructions.push(moir_instruction!(lhs = rhs));
        }
        Ok(())
    }
}

fn resolve_lhs(
    spec_builder: &ProcessorSpecificationBuilder,
    scope: &mut MOIRScope,
    local: Spanned<bool>,
    var: Spanned<SASTIdent>,
) -> SleighCompilerResult<MOIRLocalId> {
    let lhs = if local.into_value() {
        // The variable is not declared yet. We allocate it now.
        scope.allocate_named_temporary(var.clone().transmute(), MOIRMaybeSize::Undetermined)?
    } else {
        // The variable must already be declared.
        scope.resolve_ident(spec_builder, var.transmute())?
    };
    Ok(lhs)
}
