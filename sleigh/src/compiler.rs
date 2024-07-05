use std::collections::HashMap;
use std::fs;
use std::path::{
    Path,
    PathBuf,
};
use std::process::exit;

use slotmap::{
    new_key_type,
    SlotMap,
};

use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::SleighLexer;
use crate::compiler::parser::SleighParser;
use crate::preprocessor::SleighPreprocessorFactory;
use crate::spec::ProcessorSpecification;

pub mod ast;
pub mod error;
pub mod lexer;
mod parser;
pub mod tokens;

new_key_type! { struct FileStorageId; }

#[derive(Debug, Default)]
pub struct FileCache {
    files: HashMap<PathBuf, String>,
}

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer,
    parser: SleighParser,
    file_cache: FileCache,
}

#[derive(Debug)]
pub struct CompilerInput {
    origin: PathBuf,
    data: String,
}

impl<'a> CompilerInput {
    pub fn path(&'a self) -> &'a Path {
        self.origin.as_path()
    }

    pub fn data(&'a self) -> &'a str {
        self.data.as_str()
    }
}

impl TryFrom<&Path> for CompilerInput {
    type Error = SleighCompilerError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        Ok(Self {
            origin: path.to_path_buf(),
            data: fs::read_to_string(path)?,
        })
    }
}

impl SleighCompiler {
    /// Compile the provided sleigh specification.
    pub fn compile(
        &mut self,
        spec_path: &Path,
    ) -> Result<ProcessorSpecification, SleighCompilerError> {
        // let preprocessed_sleigh = self
        //     .preprocessor_factory
        //     .build()
        //     .preprocess(fs::read_to_string(spec_path)?.as_ref())?;
        // let tokens = self.lexer.tokenize(preprocessed_sleigh.as_str())?;
        let input = CompilerInput::try_from(spec_path)?;
        let tokens = self.lexer.tokenize(&input)?;
        let ast = self.parser.parse(&input, tokens);
        // self.preprocessor_factory
        //     .build()
        //     .preprocess_tokenized(tokens)?;
        println!("{ast:?}");
        exit(0);
        todo!()
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
            parser: Default::default(),
            file_cache: Default::default(),
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
