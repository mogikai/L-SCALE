use crate::compiler::build::trace_build;
use crate::compiler::{
    SleighCompiler,
    InternalCompilerResult,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROpCategory,
    MOIROperation,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::scope::MOIRMaybeSize::{
    Determined,
    Undetermined,
};
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
};

impl SleighCompiler {
    pub fn infer_sizes(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> InternalCompilerResult<()> {
        trace_build!("Doing size inference forward pass");
        forward_pass(scope, instructions)?;
        trace_build!("Doing size inference backward pass");
        backward_pass(scope, instructions)?;
        Ok(())
    }
}

fn forward_pass(
    scope: &mut MOIRScope,
    instructions: &mut Vec<MOIRInstruction>,
) -> InternalCompilerResult<()> {
    for instruction in instructions.iter_mut() {
        infer_size(scope, instruction)?;
    }
    Ok(())
}

fn backward_pass(
    scope: &mut MOIRScope,
    instructions: &mut Vec<MOIRInstruction>,
) -> InternalCompilerResult<()> {
    for instruction in instructions.iter_mut().rev() {
        infer_size(scope, instruction)?;
        let (output_size, input_a_size, input_b_size) = get_sizes(scope, instruction);
        trace_build!("Sizes are O:{output_size} A:{input_a_size} B:{input_b_size}");
        if instruction.output.is_some() && output_size.is_undetermined() {
            todo!("Generate error report: Unable to infer size of output")
        }
        if instruction.input_a.is_some() && input_a_size.is_undetermined() {
            todo!("Generate error report: Unable to infer sizes of input_a")
        }
        if instruction.input_b.is_some() && input_b_size.is_undetermined() {
            todo!("Generate error report: Unable to infer sizes of input_b")
        }
    }
    Ok(())
}

fn infer_size(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> InternalCompilerResult<()> {
    trace_build!("Inferring size of instruction {}", instruction);
    match instruction.operation.op_category() {
        MOIROpCategory::Comparison => {
            infer_comparison(scope, instruction)?;
        }
        MOIROpCategory::Shift => {
            infer_shift(scope, instruction)?;
        }
        MOIROpCategory::Regular => {
            infer_regular(scope, instruction)?;
        }
        MOIROpCategory::Branch => {
            // Probably no need to do anything?
        }
        MOIROpCategory::Special => {
            infer_special(scope, instruction)?;
        }
    }
    Ok(())
}

fn infer_comparison(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> InternalCompilerResult {
    trace_build!("Inferring comparison sizes");
    let (_, input_a_size, input_b_size) = get_sizes(scope, instruction);
    // Output of cmp is always 1 byte (true / false)
    scope.constrain_local_size(
        &mut MOIRLocal::from(
            instruction
                .output
                .expect("Comparison operations are expected to have an output"),
        ),
        1,
    )?;
    match input_a_size {
        Determined(input_a_size) => {
            scope.constrain_local_size(
                instruction
                    .input_b
                    .as_mut()
                    .expect("Comparison operations must have two inputs"),
                input_a_size,
            )?;
        }
        Undetermined => {
            if let Determined(input_b_size) = input_b_size {
                scope.constrain_local_size(
                    instruction
                        .input_a
                        .as_mut()
                        .expect("Comparison operations must have two inputs"),
                    input_b_size,
                )?;
            }
        }
    }
    Ok(())
}

fn infer_shift(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> InternalCompilerResult {
    let (output_size, input_a_size, _) = get_sizes(scope, instruction);
    match output_size {
        Determined(output_size) => {
            scope.constrain_local_size(
                instruction
                    .input_a
                    .as_mut()
                    .expect("Shift operations are expected to have an output and two inputs"),
                output_size,
            )?;
        }
        Undetermined => {
            if let Determined(input_a_size) = input_a_size {
                scope.constrain_local_size(
                    instruction
                        .input_a
                        .as_mut()
                        .expect("Shift operations are expected to have an output and two inputs"),
                    input_a_size,
                )?;
            }
        }
    }
    match instruction
        .input_b
        .expect("Shift operations are expected to have an input and two inputs")
    {
        MOIRLocal::Local(_) => {}
        MOIRLocal::Const(value) => {
            if let MOIRConst::Undetermined(value) = value {
                let size = ((value.checked_ilog2().unwrap_or(1) + 7) / 8)
                    .try_into()
                    .expect("The maximum size of 128 bit integers is 128 bit which fits into a u8");
                trace_build!("Constraining size of input_b to {size}");
                scope.constrain_local_size(
                    instruction
                        .input_b
                        .as_mut()
                        .expect("Shift operations are expected to have an output and two inputs"),
                    size,
                )?;
            }
        }
    }
    Ok(())
}

fn infer_special(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> InternalCompilerResult<()> {
    let (output_size, input_a_size, input_b_size) = get_sizes(scope, instruction);
    match instruction.operation {
        MOIROperation::Build(_) => {
            // Build operations have no size constraints since they do not contain any inputs or outputs,
            // since they are a special operation used only during constructing the final instruction sequence.
        }
        MOIROperation::Load(_) => {
            // For load we only need to make sure the address is the correct size.
            // TODO: Where do we get the correct address size from?
            // TODO: This probably needs to match with inst_next?
        }
        MOIROperation::AddressOf => {
            // Address of does not constrain the sizes, all size inference is already done upon instruction
            // creation.
        }
        MOIROperation::PopCount => {}
        MOIROperation::LZCount => {}
        MOIROperation::ZeroExtend | MOIROperation::SignExtend => {
            // We cannot do any size inference, but we can make sure that the
            // sizes are valid, i.e. that output_size >= input_size
            if let Determined(output_size) = output_size {
                if let Determined(input_a_size) = input_a_size {
                    if output_size < input_a_size {
                        todo!("Generate error report for invalid sizes for (z/s)ext")
                    }
                }
            }
        }
        MOIROperation::Carry | MOIROperation::SignCarry | MOIROperation::SignBorrow => {
            let Some(output) = instruction.output else {
                todo!("Generate error for missing output")
            };
            let Some(lhs) = &mut instruction.input_a else {
                todo!("Generate error for missing lhs")
            };
            let Some(rhs) = &mut instruction.input_b else {
                todo!("Generate error for missing rhs")
            };
            // Sizes of a and b must be equal, and out must be size 1
            scope.constrain_local_size(&mut MOIRLocal::from(output), 1)?;
            match input_a_size {
                Determined(lhs_size) => scope.constrain_local_size(rhs, lhs_size)?,
                Undetermined => {
                    if let Determined(rhs_size) = input_b_size {
                        scope.constrain_local_size(lhs, rhs_size)?
                    }
                }
            }
        }
        MOIROperation::Nan => {
            let Some(output) = instruction.output else {
                todo!("Generate error for missing output")
            };
            // Sizes of out must be size 1
            scope.constrain_local_size(&mut MOIRLocal::from(output), 1)?;
        }
        MOIROperation::Abs => {}
        MOIROperation::Sqrt => {}
        MOIROperation::IntToFloat => {}
        MOIROperation::FloatToFloat => {}
        MOIROperation::Trunc => {}
        MOIROperation::Ceil => {}
        MOIROperation::Floor => {}
        MOIROperation::Round => {
            // Input and output for round have to have the same size
            let size = output_size.or(input_a_size);
            if let Determined(size) = size {
                if let Some(output) = instruction.output {
                    scope.constrain_local_size(&mut MOIRLocal::from(output), size)?;
                }
                if let Some(input_a) = instruction.input_a.as_mut() {
                    scope.constrain_local_size(input_a, size)?;
                }
            }
        }
        _ => {
            // There is nothing to do in this case.
        }
    }
    Ok(())
}

fn infer_regular(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> InternalCompilerResult<()> {
    let (output_size, input_a_size, input_b_size) = get_sizes(scope, instruction);
    // For regular operations all sizes have to match.
    // Check if any part of the operation has a determined size.
    let size = output_size.or(input_a_size).or(input_b_size).or_else(|| {
        // Check if there is a constant from which we could try to infer at least a minimum required size
        instruction
            .input_a
            .as_ref()
            .map_or(Undetermined, |input_a| get_minimum_const_size(input_a))
            .or(instruction
                .input_b
                .as_ref()
                .map_or(Undetermined, |input_b| get_minimum_const_size(input_b)))
    });
    if let Determined(size) = size {
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
    Ok(())
}

fn get_sizes(
    scope: &mut MOIRScope,
    instruction: &mut MOIRInstruction,
) -> (MOIRMaybeSize, MOIRMaybeSize, MOIRMaybeSize) {
    let output_size = instruction
        .output
        .map(|v| scope.size_of_local(v))
        .unwrap_or(Undetermined);
    let input_a_size = instruction
        .input_a
        .map(|v| v.size(scope))
        .unwrap_or(Undetermined);
    let input_b_size = instruction
        .input_b
        .map(|v| v.size(scope))
        .unwrap_or(Undetermined);
    (output_size, input_a_size, input_b_size)
}

/// Returns the minimum required size for a const local
fn get_minimum_const_size(value: &MOIRLocal) -> MOIRMaybeSize {
    match value {
        MOIRLocal::Local(_) => Undetermined,
        MOIRLocal::Const(value) => match value {
            MOIRConst::Undetermined(value) => MOIRSize::try_from((value.checked_ilog2().unwrap_or(1) + 7) / 8)
                .expect("The maximum size of 128 bit integers is 128 bit which fits into a u8")
                .into(),
            _ => value.size(),
        },
    }
}
