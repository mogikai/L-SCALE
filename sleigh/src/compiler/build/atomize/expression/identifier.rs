use crate::compiler::build::trace_build;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use mogi_sleigh_parser::{
    Spanned,
    StringId,
};

impl SleighCompiler {
    /// Atomizing an identifier always returns a local since we don't want to simply copy this into an out local.
    /// Hence, we also just need a size constraint if applicable.
    pub fn atomize_identifier(
        &mut self,
        scope: &mut MOIRScope,
        size_constraint: MOIRMaybeSize,
        ident: Spanned<StringId>,
    ) -> InternalCompilerResult<MOIRLocal> {
        trace_build!("Atomizing identifier with size_constraint {size_constraint:?}");
        match size_constraint {
            MOIRMaybeSize::Determined(size) => {
                // The size constraint is determined. Update the size in the scope.
                // TODO map error here to add new information? See generate functions before this commit
                scope
                    .resolve_ident_sized(&self.spec_builder, ident, size)
                    .map(MOIRLocal::from)
            }
            MOIRMaybeSize::Undetermined => {
                // Size constraint is undetermined. We can just return the resolved value.
                Ok(scope.resolve_ident(&self.spec_builder, ident)?.into())
            }
        }
    }
}
