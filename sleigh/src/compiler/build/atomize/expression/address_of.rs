use crate::compiler::ast::pcode::addressof::SASTPCodeAddressOf;
use crate::compiler::ast::Spanned;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
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
    /// Translate the address_of operation. The sleigh specification allows taking the
    /// address of a varnode in the const space. This however translates directly
    /// to just providing the constant value, since the sleigh spec says that the offset
    /// of a varnode in the const address space corresponds to its value.
    /// I.e. the address of varnode {space: id_const, offset: 42, size: 4} translates to
    /// the constant value 42 with size 4.
    /// The address of and deref operations on values in the const address space or on
    /// constant values are translated on constructor invocation into their constant values.
    ///
    /// Quote from pcoderef.html:
    /// If a varnode is specified as an offset into the constant address space, that
    /// offset is interpreted as a constant, or immediate value, in any p-code operation
    /// that uses that varnode. The size of the varnode, in this case, can be treated
    /// as the size or precision available for the encoding of the constant.
    pub fn atomize_address_of(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        address_of: Spanned<SASTPCodeAddressOf>,
    ) -> InternalCompilerResult<MOIRLocal> {
        let (address_of, _) = address_of.into_tuple();
        let size = address_of
            .size
            .map(|s| MOIRMaybeSize::Determined(s.into_value()))
            .unwrap_or(MOIRMaybeSize::Undetermined);
        if let MOIRMaybeSize::Determined(size_constraint) = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined)
        {
            // If out has a size and the address_of operator also specifies
            // a size, they have to match.
            if let MOIRMaybeSize::Determined(size) = size {
                if size != size_constraint {
                    todo!("Generate error report for mismatched sizes")
                }
            }
        }
        // The size of rhs is not relevant, so we leave it up to the atomization to allocate if necessary.
        let rhs = self.atomize_expression(scope, instructions, None, *address_of.expression)?;
        let lhs = out.unwrap_or_else(|| scope.allocate_temporary(size));
        instructions.push(moir_instruction!(lhs = &rhs));
        Ok(lhs.into())
    }
}
