use crate::compiler::ast::pcode::deref::SASTPCodeDeref;
use crate::compiler::ast::Spanned;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
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
use std::cmp::Ordering;
use tracing::trace;

impl SleighCompiler {
    pub fn atomize_load(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> SleighCompilerResult<MOIRLocal> {
        trace!("Atomizing load expression");
        let (deref, _) = deref.into_tuple();
        let value_size = deref
            .size
            .map(Spanned::into_value)
            .map(MOIRMaybeSize::Determined)
            .unwrap_or(MOIRMaybeSize::Undetermined);
        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);

        // If out is not specified we allocate our own out variable to store the result in.
        let out = out.unwrap_or_else(|| scope.allocate_temporary(out_size.or(value_size)));
        let space = self
            .spec_builder
            .resolve_space(deref.space.map(Spanned::transmute))?;
        let addr = self.atomize_expression(scope, instructions, None, *deref.expression)?;
        if let MOIRMaybeSize::Determined(out_size) = out_size {
            if let MOIRMaybeSize::Determined(value_size) = value_size {
                match out_size.cmp(&value_size) {
                    Ordering::Less => {
                        todo!("Generate error for out too small for loaded value")
                    }
                    Ordering::Equal => {
                        // If the size of out is equal to the size of the load, we can just perform the load.
                        instructions.push(moir_instruction!(out = *[space]addr));
                    }
                    Ordering::Greater => {
                        // If the size of out is larger than the loaded value, we perform a zero extend.
                        // Insert the operation here. The jit might optimize the zero extend + load into one op.
                        // Allocate tmp for loading the value into a correctly sized varnode.
                        let tmp = scope.allocate_temporary(value_size);
                        // Load the value
                        instructions.push(moir_instruction!(tmp = *[space]addr));
                        instructions.push(moir_instruction!(out = zext(tmp)));
                    }
                }
            }
        }
        Ok(out.into())
    }
}
