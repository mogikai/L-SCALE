use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::Spanned;
use crate::compiler::build::trace_build;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::moir::r#macro::MOIRMacro;

impl SleighCompiler {
    pub fn process_pcode_macro(
        &mut self,
        pcode_macro: Spanned<SASTPCodeMacro>,
    ) -> InternalCompilerResult {
        trace_build!("Processing pcode macro at {:?}", pcode_macro.span());
        let (pcode_macro, _) = pcode_macro.into_tuple();

        let moir_macro = MOIRMacro {
            arguments: pcode_macro.arguments.into_value(),
            pcode: pcode_macro.pcode.into_value(),
        };

        let result = self
            .spec_builder
            .define_macro(pcode_macro.ident.transmute(), moir_macro);
        self.report_error_or_unwrap(result)?;
        Ok(())
    }
}
