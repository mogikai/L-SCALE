use crate::compiler::ast::pcode::invocation::SASTPCodeInvocation;
use crate::compiler::ast::Spanned;
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

        let args = invocation
            .arguments
            .into_iter()
            .map(|arg| self.atomize_expression(scope, instructions, None, arg))
            .collect::<Result<Vec<_>, _>>()?;

        let name = invocation.name.transmute();
        if let Some(moir_macro) = self.spec_builder.macro_by_ident(&name).cloned() {
            scope.with_empty_namespace(|scope| {
                for (arg_local, arg_name) in args.iter().zip(
                    moir_macro
                        .arguments
                        .into_iter()
                        .map(Spanned::transmute::<StringId>),
                ) {
                    match arg_local {
                        MOIRLocal::Local(local) => {
                            scope.declare_local_reference(arg_name, *local)?;
                        }
                        MOIRLocal::Const(value) => {
                            // Constants need to be copied into a named temporary
                            let arg = scope.allocate_named_temporary(arg_name, value.size())?;
                            instructions.push(moir_instruction!(arg = { *value }));
                        }
                    }
                }
                let export = self.atomize_statements(scope, instructions, moir_macro.pcode.statements)?;
                if export.is_some() {
                    todo!("Generate error report for export used in macro where it is not allowed")
                }
                Ok(None)
            })
        } else {
            let output = out.unwrap_or_else(|| scope.allocate_temporary(MOIRMaybeSize::Undetermined));
            instructions.push(MOIRInstruction {
                output: Some(output),
                input_a: args.first().copied(),
                input_b: args.get(1).copied(),
                operation: MOIROperation::CustomOp(
                    args.split_at_checked(2)
                        .map(|(_, args)| args.to_vec())
                        .unwrap_or(vec![]),
                ),
                label: None,
            });
            Ok(Some(output.into()))
        }
    }
}
