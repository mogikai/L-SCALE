use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};

impl SleighCompiler {
    pub fn atomize_assignment_store(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<()> {
        if *local {
            todo!("Generate error report for local keyword on store assignment")
        }
        let (deref, _) = deref.into_tuple();

        let addr = self.atomize_expression(scope, instructions, None, *deref.expression)?;
        let space = self
            .spec_builder
            .resolve_space(deref.space.map(Spanned::transmute))?;
        let value_size = deref
            .size
            .map(Spanned::into_value)
            .map(MOIRMaybeSize::Determined)
            .unwrap_or(MOIRMaybeSize::Undetermined);
        let mut rhs = self.atomize_expression(scope, instructions, None, expression)?;
        if let MOIRMaybeSize::Determined(constraint_size) = value_size {
            if rhs.size(scope).is_determined() {
                // If rhs has determined size,  we might need to truncate.
                // This generates an error if rhs.size > constraint_size
                rhs = rhs.truncate(scope, constraint_size)?;
            } else {
                // If rhs has undetermined size, and the store provides a size, we can constrain the rhs here.
                scope.constrain_local_size(&mut rhs, constraint_size)?;
            }
        }

        instructions.push(moir_instruction!(*[space]addr = rhs));

        Ok(())
    }
}
