use crate::compiler::build::trace_build;
use crate::compiler::error::InternalError;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::pcode::deref::SASTPCodeDeref;
use std::cmp::Ordering;

impl SleighCompiler {
    pub fn atomize_load(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        deref: Spanned<SASTPCodeDeref>,
    ) -> InternalCompilerResult<MOIRLocal> {
        trace_build!("Atomizing load expression {deref:?}");
        let (deref, _) = deref.into_tuple();

        let value_size = deref
            .size
            .map(Spanned::into_value)
            .map(|size| size.try_into().expect("The size must fit into u8"))
            .map(MOIRMaybeSize::Determined)
            .unwrap_or(MOIRMaybeSize::Undetermined);
        trace_build!("Load size is {value_size:?}");

        let out_size = out
            .map(|id| scope.size_of_local(id))
            .unwrap_or(MOIRMaybeSize::Undetermined);
        trace_build!("Out size is {out_size:?}");

        // If out is not specified we allocate our own out variable to store the result in.
        let out = out.unwrap_or_else(|| scope.allocate_temporary(out_size.or(value_size)));
        // If a value size is specified, then out must be constrained to this size.
        if let MOIRMaybeSize::Determined(value_size) = value_size {
            if scope.size_of_local(out).is_undetermined() {
                // We only want to constrain the size if it is undetermined.
                // The spec does some wonky stuff here, and allows implicit
                // zero extends. See below...
                scope.constrain_local_size(&mut MOIRLocal::from(out), value_size)?;
            }
        }
        let MOIRMaybeSize::Determined(out_size) = scope.size_of_local(out) else {
            todo!("Generate error for missing size constraint for load")
        };
        let space = self
            .spec_builder
            .resolve_space(deref.space.map(Spanned::transmute))?;
        let mut addr = self.atomize_expression(scope, instructions, None, *deref.expression)?;
        let spc = self
            .spec_builder
            .space_by_symbol_id(space)
            .ok_or_else(|| -> InternalError { todo!("Generate appropriate error report") })?;

        trace_build!("Constraining addr size to {}", spc.address_size);
        scope.constrain_local_size(&mut addr, spc.address_size)?;

        let MOIRMaybeSize::Determined(value_size) = value_size.or(out_size.into()) else {
            todo!("This should not happen. What does it mean when both sizes are not set?")
        };
        match out_size.cmp(&value_size) {
            Ordering::Less => {
                todo!("Generate error for out too small for loaded value")
            }
            Ordering::Equal => {
                // If the size of out is equal to the size of the load, we can just perform the load.
                instructions.push(moir_instruction!(out = *[space]addr));
            }
            Ordering::Greater => {
                // TODO: The spec is underdefined in this regard, or the default compiler is wrong.
                //       The output size should be the same as the one specified here.
                //       This implicit zero extend is kind of stupid imho.
                // If the size of out is larger than the loaded value, we perform a zero extend.
                // Insert the operation here. The jit might optimize the zero extend + load into one op.
                // Allocate tmp for loading the value into a correctly sized varnode.
                let tmp = scope.allocate_temporary(value_size);
                // Load the value
                instructions.push(moir_instruction!(tmp = *[space]addr));
                instructions.push(moir_instruction!(out = zext(tmp)));
            }
        }
        Ok(out.into())
    }
}
