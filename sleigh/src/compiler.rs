pub mod error;
mod lexer;
mod tokens;

use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    lexer,
    SleighLexer,
};
use crate::preprocessor::{
    SleighPreprocessor,
    SleighPreprocessorFactory,
};
use crate::spec::ProcessorSpecification;
use std::fs;
use std::path::Path;
use std::process::exit;

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer,
}

impl SleighCompiler {
    /// Compile the provided sleigh specification.
    pub fn compile(
        &self,
        spec_path: &Path,
    ) -> Result<ProcessorSpecification, SleighCompilerError> {
        // let preprocessed_sleigh = self
        //     .preprocessor_factory
        //     .build()
        //     .preprocess(fs::read_to_string(spec_path)?.as_ref())?;
        // let tokens = self.lexer.tokenize(preprocessed_sleigh.as_str())?;
        let input = fs::read_to_string(spec_path)?;
        let tokens = self.lexer.tokenize(input.as_str())?;
        println!("{tokens:?}");
        exit(0);
        todo!()
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
        }
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
