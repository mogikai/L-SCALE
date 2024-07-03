pub mod error;

use crate::compiler::error::SleighCompilerError;
use crate::preprocessor::{
    SleighPreprocessor,
    SleighPreprocessorFactory,
};
use crate::spec::ProcessorSpecification;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
}

impl SleighCompiler {
    /// Compile the provided sleigh specification.
    pub fn compile(
        &self,
        spec_path: &Path,
    ) -> Result<ProcessorSpecification, SleighCompilerError> {
        let spec_data = self
            .preprocessor_factory
            .build()
            .preprocess(fs::read_to_string(spec_path)?.as_ref())?;
        todo!()
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self { preprocessor_factory }
    }
}

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
        SleighCompiler::new(self.preprocessor_factory.unwrap_or(Default::default()))
    }
}
