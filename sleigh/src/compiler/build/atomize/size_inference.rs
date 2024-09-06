use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROpCategory,
    MOIROperation,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use tracing::trace;

impl SleighCompiler {
    pub fn infer_sizes(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> SleighCompilerResult<()> {
        trace!("Doing size inference pass");
        for instruction in instructions.iter_mut() {
            trace!("{instruction}");
            let output_size = instruction
                .output
                .map(|v| scope.size_of_local(v))
                .unwrap_or(MOIRMaybeSize::Undetermined);
            let input_a_size = instruction
                .input_a
                .map(|v| v.size(scope))
                .unwrap_or(MOIRMaybeSize::Undetermined);
            let input_b_size = instruction
                .input_b
                .map(|v| v.size(scope))
                .unwrap_or(MOIRMaybeSize::Undetermined);
            trace!("{output_size:?} {input_a_size:?} {input_b_size:?}");
            match instruction.operation.op_category() {
                MOIROpCategory::Comparison => {}
                MOIROpCategory::Shift => {}
                MOIROpCategory::Regular => {
                    // For regular operations all sizes have to match.
                    // Check if any part of the operation has a determined size.
                    let size = output_size.or(input_a_size).or(input_b_size);
                    if let MOIRMaybeSize::Determined(size) = size {
                        // If we have a size, we can constrain all values to this size.
                        // Otherwise, we have to wait until the backwards pass.
                        if let Some(output) = instruction.output {
                            scope.constrain_local_size(&mut MOIRLocal::from(output), size)?;
                        }
                        if let Some(local) = instruction.input_a.as_mut() {
                            scope.constrain_local_size(local, size)?
                        }
                        if let Some(local) = instruction.input_b.as_mut() {
                            scope.constrain_local_size(local, size)?
                        }
                    }
                }
                MOIROpCategory::Special => {
                    match instruction.operation {
                        MOIROperation::Load(_) => {
                            // For load we only need to make sure the address is the correct size.
                            // TODO: Where do we get the correct address size from?
                            // TODO: This probably needs to match with inst_next?
                        }
                        MOIROperation::ZeroExtend => {
                            // We cannot do any size inference, but we can make sure that the
                            // sizes are valid, i.e. that output_size > input_size
                            if let MOIRMaybeSize::Determined(output_size) = output_size {
                                if let MOIRMaybeSize::Determined(input_a_size) = input_a_size {
                                    if output_size <= input_a_size {
                                        todo!("Generate error report for invalid sizes for zext")
                                    }
                                }
                            }
                        }
                        _ => {
                            // There is nothing to do in this case.
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
