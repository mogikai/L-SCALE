use crate::compiler::ast::pcode::subpiece::{
    SASTPCodeSubpiece,
    SASTPCodeSubpieceSelector,
    SASTPCodeSubpieceTarget,
};
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;

impl SleighCompiler {
    /// The subpiece operation throws away the lower N bits.
    /// The resulting size of the output is thus (size - N).
    /// Since we simply transform a local with this atomization,
    /// we only need a size constraint and no out node and also
    /// always return a local.
    pub fn atomize_subpiece(
        &mut self,
        scope: &mut MOIRScope,
        size_constraint: MOIRMaybeSize,
        subpiece: Spanned<SASTPCodeSubpiece>,
    ) -> SleighCompilerResult<MOIRLocal> {
        let (subpiece, _) = subpiece.into_tuple();
        let (offset, size) = match subpiece.selector.into_value() {
            SASTPCodeSubpieceSelector::Take(size) => {
                // If we take n, the offset is zero, and the size is determined as n
                (0, size)
            }
            SASTPCodeSubpieceSelector::Discard(offset) => {
                // If we discard n, then the offset is n and the size is undetermined,
                // i.e. must be determined by the external size constraint.
                (
                    offset,
                    size_constraint.ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error for missing size constraint")
                    })?,
                )
            }
        };
        let mut value = match subpiece.target.into_value() {
            SASTPCodeSubpieceTarget::Ident(ident) => {
                // We provide undetermined size here, because we do not want to constrain the underlying value.
                // We can also not create a subpiece, if the underlying value has undetermined size.
                self.atomize_identifier(scope, MOIRMaybeSize::Undetermined, ident.transmute())
            }
            SASTPCodeSubpieceTarget::Literal(literal) => {
                // For literal resolution, a size is provided, since we want to make sure that the value of which
                // the size is specified fits.
                self.atomize_literal(size.into(), literal)
            }
        }?;
        // If the size is not determined yet, we can constrain it here to the same size as the slice.
        if value.size(scope).is_undetermined() {
            scope.constrain_local_size(&mut value, size)?;
        }
        // We can now slice the value to the desired size.
        // TODO: Generate error when slice fails because of too large sizes.
        value.slice(scope, offset, offset + size)
    }
}
