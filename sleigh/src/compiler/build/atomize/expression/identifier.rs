use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::report::generate_invalid_sizes_error;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use tracing::trace;

impl SleighCompiler {
    /// Atomizing an identifier always returns a local since we don't want to simply copy this into an out local.
    /// Hence, we also just need a size constraint if applicable.
    pub fn atomize_identifier(
        &mut self,
        scope: &mut MOIRScope,
        size_constraint: MOIRMaybeSize,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing identifier with size_constraint {size_constraint:?}");
        match size_constraint {
            MOIRMaybeSize::Determined(size) => {
                // The size constraint is determined. Update the size in the scope.
                scope
                    .resolve_ident_sized(&self.spec_builder, ident.clone(), size)
                    .map(MOIRLocal::from)
                    .map_err(|e| match e {
                        // TODO: Refactor into error helper function and generate better reports
                        SleighCompilerError::InvalidSliceSize { new, current } => {
                            generate_invalid_sizes_error(ident.into_span(), current, new)
                        }
                        SleighCompilerError::InvalidSliceRange {
                            current_start,
                            current_end,
                            new_start,
                            new_end,
                        } => generate_invalid_sizes_error(
                            ident.into_span(),
                            current_end - current_start,
                            new_end - new_start,
                        ),
                        SleighCompilerError::CannotSliceUndetermined => {
                            generate_invalid_sizes_error(ident.into_span(), 0, size)
                        }
                        e => e,
                    })
            }
            MOIRMaybeSize::Undetermined => {
                // Size constraint is undetermined. We can just return the resolved value.
                Ok(scope.resolve_ident(&self.spec_builder, ident)?.into())
            }
        }
    }
}
