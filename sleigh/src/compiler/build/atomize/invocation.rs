use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::expression::SASTPCodeExpression;
use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::StringId;
use crate::spec::moir::code::{
    moir_instruction,
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::custom_op::MOIRCustomOp;
use crate::spec::moir::r#macro::MOIRMacro;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::varnode::MOIRLocal;
use tracing::trace;

impl SleighCompiler {
    pub fn atomize_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        invocation: Spanned<SASTPCodeInvocation>,
    ) -> SleighCompilerResult<Option<MOIRLocal>> {
        trace!("Atomizing invocation with {} arguments.", invocation.value().arguments.len());
        let (invocation, _) = invocation.into_tuple();

        let args = self.atomize_invocation_arguments(scope, instructions, invocation.arguments)?;

        let name = invocation.name.transmute();
        if let Some(moir_macro) = self.spec_builder.macro_by_ident(&name).cloned() {
            self.atomize_macro_invocation(scope, instructions, args, moir_macro)
        } else {
            self.atomize_custom_op_invocation(scope, instructions, out, args, name)
        }
    }

    fn atomize_custom_op_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        out: Option<MOIRLocalId>,
        mut args: Vec<MOIRLocal>,
        name: Spanned<StringId>,
    ) -> SleighCompilerResult<Option<MOIRLocal>> {
        let Some(custom_op) = self.spec_builder.custom_op_by_ident_mut(&name) else {
            trace!("Missing custom op {}", self.spec_builder.resolve_string_id(name.into_value()));
            todo!("Generate error for missing custom op")
        };
        let arg_sizes = args.iter().map(|arg| arg.size(scope)).collect::<Vec<_>>();
        match custom_op {
            MOIRCustomOp::Declared => {
                *custom_op = MOIRCustomOp::Defined {
                    input_constraints: arg_sizes,
                    output: out.map(|local| scope.size_of_local(local)),
                }
            }
            MOIRCustomOp::Defined {
                input_constraints,
                output,
            } => {
                for ((arg, constraint), size) in args
                    .iter_mut()
                    .zip(input_constraints.into_iter())
                    .zip(arg_sizes)
                {
                    if let MOIRMaybeSize::Determined(constraint) = constraint {
                        if let MOIRMaybeSize::Determined(size) = size {
                            // both are defined and need to be equal
                            if *constraint != size {
                                todo!("Generate error for invalid size constraints in invocation")
                            }
                        } else {
                            // constraint is defined but size isn't. We can constrain size now
                            scope.constrain_local_size(arg, *constraint)?
                        }
                    } else {
                        if let MOIRMaybeSize::Determined(size) = size {
                            // Constraint is undetermined, but size is. we can determine the constraint now
                            *constraint = size.into();
                        }
                        // If both are undetermined we don't need to do anything.
                    }
                }
            }
        }
        let op_id = self.spec_builder.custom_op_id_by_ident(&name)?;
        instructions.push(MOIRInstruction {
            output: out,
            input_a: args.first().copied(),
            input_b: args.get(1).copied(),
            operation: MOIROperation::CustomOp {
                op_id,
                extra_args: args
                    .split_at_checked(2)
                    .map(|(_, args)| args.to_vec())
                    .unwrap_or(vec![]),
            },
            label: None,
        });
        Ok(out.map(MOIRLocal::from))
    }

    /// Atomizes each argument expression such that at the end each argument is stored in its own local.
    /// This vec of locals is then returned.
    fn atomize_invocation_arguments(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        arguments: Vec<Spanned<SASTPCodeExpression>>,
    ) -> SleighCompilerResult<Vec<MOIRLocal>> {
        arguments
            .into_iter()
            .map(|arg| self.atomize_expression(scope, instructions, None, arg))
            .collect::<Result<Vec<_>, _>>()
    }

    fn atomize_macro_invocation(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &mut Vec<MOIRInstruction>,
        args: Vec<MOIRLocal>,
        moir_macro: MOIRMacro,
    ) -> SleighCompilerResult<Option<MOIRLocal>> {
        // We need to create a new namespace that is only used for this macro invocation.
        // Effectively, the macro just writes its instructions into the same scope and
        // instruction list as that of the constructor, but uses its own name resolution
        // tables. These tables are discarded after the macro is constructed, since they
        // are no longer needed.
        scope.with_empty_namespace(|scope| {
            process_macro_arguments(scope, instructions, args, moir_macro.arguments)?;
            // Process the individual statements of the macro and insert them in the instructions list.
            let export = self.atomize_statements(scope, instructions, moir_macro.pcode.statements)?;
            if export.is_some() {
                todo!("Generate error report for export used in macro where it is not allowed")
            }
            Ok(None)
        })
    }
}

/// Resolves the provided args to the argument names in the macro definition.
///
/// This function requires that the namespace in the scope is clear, or at least
/// has none of the arguments already defined. Otherwise, this will produce an error.
///
/// # Arguments
///
/// * `scope`: The local scope to define the arguments in
/// * `instructions`: The instructions vector to append new instructions to if needed.
/// * `args`: A vec of arguments that are passed into the macro
/// * `arg_definitions`: A vec of argument definitions, as defined in the macro definition.
///
/// returns: Ok(()) if all went well and an error if an argument could not be resolved correctly.
fn process_macro_arguments(
    scope: &mut MOIRScope,
    instructions: &mut Vec<MOIRInstruction>,
    args: Vec<MOIRLocal>,
    arg_definitions: Vec<Spanned<SASTIdent>>,
) -> SleighCompilerResult {
    let args_and_names = args.iter().zip(
        arg_definitions
            .into_iter()
            .map(Spanned::transmute::<StringId>),
    );

    for (arg, name) in args_and_names {
        match arg {
            MOIRLocal::Local(local) => scope.declare_local_reference(name, *local)?,
            MOIRLocal::Const(value) => {
                // Constants need to be copied into a named temporary
                let arg = scope.allocate_named_temporary(name, value.size())?;
                instructions.push(moir_instruction!(arg = { *value }));
            }
        }
    }
    Ok(())
}
