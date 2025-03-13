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
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::common::{
    SASTIdent,
    SASTSize,
};
use mogi_sleigh_parser::ast::pcode::bitrange::SASTPCodeBitRange;
use mogi_sleigh_parser::ast::pcode::expression::SASTPCodeExpression;
use std::cmp::Ordering;

impl SleighCompiler {
    /// Atomize a bitrange assignment statement
    /// This consists of atomizing the value expression and then shifting the resulting value
    /// into the correct position. The section of bits where the value is going to be placed
    /// is masked out, and then ored with the new value that has been shifted to the correct
    /// position, with the remaining bits being zero.
    pub fn atomize_assignment_bitrange(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        local: Spanned<bool>,
        expression: Spanned<SASTPCodeExpression>,
        bitrange: Spanned<SASTPCodeBitRange>,
    ) -> InternalCompilerResult<()> {
        let (bitrange, _) = bitrange.into_tuple();

        let shift_amount = bitrange.lsb.into_value();
        let bits = bitrange.bits.into_value();

        let minimum_value_size = ((bits + 7) / 8).try_into().expect("bits must fit into u8");
        let target = resolve_target(&self.spec_builder, scope, local, bitrange.ident)?;
        let target_size = scope
            .size_of_local(target)
            .determined_or(minimum_value_size);

        let expr_span = expression.span().clone();
        let value = self.aab_resolve_value(scope, instructions, minimum_value_size, target_size, expression)?;
        let mask =
            create_mask(scope, instructions, shift_amount, bits, target_size).map_err(|e| e.span("expr", expr_span))?;

        // We need to shift the value to the correct position before assigning.
        instructions.push(moir_instruction!(value = value << shift_amount));
        // Zero the target area.
        instructions.push(moir_instruction!(target = target & mask));
        // Copy the bits by oring the target and value. Target is zeroed where the value is located.
        // The value is zero everywhere else, thereby effectively copying value into the specified bits in target.
        instructions.push(moir_instruction!(target = target | value));

        Ok(())
    }

    /// Resolves the value expression and extends the resulting value to the target size if needed.
    fn aab_resolve_value(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        minimum_value_size: MOIRSize,
        target_size: MOIRSize,
        expression: Spanned<SASTPCodeExpression>,
    ) -> InternalCompilerResult<MOIRLocalId> {
        // Resolve the expression to its own local to allow the size to be whatever it wants to be.
        let value_local = scope.allocate_temporary(MOIRMaybeSize::Undetermined);
        let mut value = self.atomize_expression(scope, instructions, Some(value_local), expression)?;

        let size = if let MOIRMaybeSize::Determined(size) = value.size(scope) {
            size
        } else {
            // If the size is not determined, we try to constrain the size to the minimum needed value.
            scope.constrain_local_size(&mut value, minimum_value_size)?;
            minimum_value_size
        };

        match size.cmp(&target_size) {
            Ordering::Less => {
                // The target is larger, so we have to zero extend.
                let value_local = scope.allocate_temporary(target_size);
                instructions.push(moir_instruction!(value_local = zext(value)));
                Ok(value_local)
            }
            Ordering::Equal => {
                if value_local != value {
                    // If the value was not stored in the allocated local, we need to copy it.
                    scope.constrain_local_size(&mut MOIRLocal::from(value_local), target_size)?;
                    instructions.push(moir_instruction!(value_local = value));
                }
                Ok(value_local)
            }
            Ordering::Greater => {
                todo!("Generate an error for value size being too large for target?")
            }
        }
    }
}

/// Resolve the target of the bitrange assignment. Declare a local if local is true, and
/// otherwise resolve an existing local.
fn resolve_target(
    spec_builder: &ProcessorSpecificationBuilder,
    scope: &mut MOIRScope,
    local: Spanned<bool>,
    ident: Spanned<SASTIdent>,
) -> InternalCompilerResult<MOIRLocalId> {
    if local.into_value() {
        scope.allocate_named_temporary(ident.clone().transmute(), MOIRMaybeSize::Undetermined)
    } else {
        scope.resolve_ident(spec_builder, ident.transmute())
    }
}

fn create_mask(
    scope: &mut MOIRScope,
    instructions: &mut Vec<MOIRInstruction>,
    shift_amount: SASTSize,
    bits: SASTSize,
    target_size: MOIRSize,
) -> InternalCompilerResult<MOIRLocal> {
    // We can only create a mask constant, if it is smaller than 128 bits, i.e. 16 bytes.
    if target_size < 16 {
        // create `bits` amount of ones and shift them left to the correct position.
        let mask = MOIRLocal::from(!(((1u128 << bits) - 1) << shift_amount));
        // Reduce the mask size to the size of target.
        mask.truncate(scope, target_size)
    } else {
        // TODO: Maybe we can construct the mask piecewise and use a byte vec const and copy that over?
        //       For now, we just do it in pcode.
        // We have to construct the mask in pcode, since the values are too large
        let mask = scope.allocate_temporary(target_size);
        instructions.push(moir_instruction!(mask = { MOIRConst::Determined(1, target_size) }));
        instructions.push(moir_instruction!(mask = mask << bits));
        instructions.push(moir_instruction!(mask = mask - { MOIRConst::new_with_size(1, target_size)? }));
        instructions.push(moir_instruction!(mask = mask << shift_amount));
        instructions.push(moir_instruction!(mask = ~mask));
        Ok(mask.into())
    }
}
