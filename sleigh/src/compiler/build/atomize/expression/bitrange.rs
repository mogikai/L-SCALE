use crate::compiler::ast::Spanned;
use crate::compiler::ast::pcode::bitrange::SASTPCodeBitRange;
use crate::compiler::{
    SleighCompiler,
    InternalCompilerResult,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};

impl SleighCompiler {
    pub fn atomize_bitrange(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        bitrange: Spanned<SASTPCodeBitRange>,
    ) -> InternalCompilerResult<MOIRLocal> {
        let (bitrange, _) = bitrange.into_tuple();
        let value = scope.resolve_ident(&self.spec_builder, bitrange.ident.transmute())?;
        let shift_amount = MOIRLocal::from(bitrange.lsb.into_value());
        let bits = bitrange.bits.into_value();
        // The size of the output is the smallest size in bytes that still fits the output.
        let size = (bits + 7) / 8;

        // Allocate an out local if none was provided. Otherwise, we need to make sure,
        // that the size of out is correct.
        let out = if let Some(out) = out {
            scope.constrain_local_size(&mut MOIRLocal::from(out), size)?;
            out
        } else {
            scope.allocate_temporary(size)
        };

        // Bitrange is an emulated instruction. We first shift the bits right until we have the lsb at position 0
        // We do not want to modify the original value, so we need to create a copy here.
        let tmp = scope.allocate_temporary(scope.size_of_local(value));
        instructions.push(moir_instruction!(tmp = value >> shift_amount));

        // We can now reduce the size of the slice, discarding the remaining bits.
        let tmp = scope.truncate_local(tmp, size)?;
        instructions.push(moir_instruction!(out = tmp));

        // And then mask out the bits that remain which we don't need.
        // Truncate the mask to correct size before storing it in a local
        let mask = if size < 16 {
            // The mask constant can be calculated internally. If it is too large we have to calculate in pcode.
            MOIRLocal::from(MOIRConst::from((1u128 << bits) - 1).truncate(size)?)
        } else {
            // The mask is too large, calculate in pcode.
            // TODO: We might be able to do this as constant calculation in the future.
            let mask = scope.allocate_temporary(size);
            instructions.push(moir_instruction!(mask = { MOIRConst::new_with_size(1, size)? } << bits));
            instructions.push(moir_instruction!(mask = mask - { MOIRConst::new_with_size(1, size)? }));
            mask.into()
        };
        instructions.push(moir_instruction!(out = out & mask));
        Ok(out.into())
    }
}
