use crate::compiler::SleighCompiler;
use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;

#[derive(Default, Debug)]
pub struct SleighCompilerBuilder {
    preprocessor_factory: Option<SleighPreprocessorFactory>,
}

impl SleighCompilerBuilder {
    pub fn new() -> Self {
        Self {
            preprocessor_factory: None,
        }
    }

    pub fn preprocessor_factory(
        mut self,
        preprocessor_factory: SleighPreprocessorFactory,
    ) -> SleighCompilerBuilder {
        self.preprocessor_factory = Some(preprocessor_factory);
        self
    }

    pub fn build(self) -> SleighCompiler {
        SleighCompiler::new(self.preprocessor_factory.unwrap_or_default())
    }
}
