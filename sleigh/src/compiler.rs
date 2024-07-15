use std::collections::HashMap;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::fs;
use std::path::{
    Path,
    PathBuf,
};
use std::process::exit;

use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::SleighLexer;
use crate::compiler::parser::SleighParser;
use crate::preprocessor::SleighPreprocessorFactory;
use crate::spec::ProcessorSpecification;
use slotmap::{
    new_key_type,
    SlotMap,
};
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::{
    StringInterner,
    Symbol,
};

pub mod ast;
pub mod error;
pub mod lexer;
mod parser;
pub mod tokens;

new_key_type! { pub struct InputId; }

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct StringId(usize);

impl Display for InputId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, Default)]
pub struct CompilerContext {
    interner: StringInterner<string_interner::backend::StringBackend>,
}

impl CompilerContext {
    /// Intern the provided value that can be converted to a string.
    /// If the value already exists, a key to that value is returned.
    /// Otherwise, a new entry is created and the key to that is returned.
    pub fn intern<T: AsRef<str>>(
        &mut self,
        string: &T,
    ) -> StringId {
        StringId(self.interner.get_or_intern(string).to_usize())
    }

    pub fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.interner
            .resolve(<<StringBackend as Backend>::Symbol as Symbol>::try_from_usize(string_id.0)?)
    }
}

#[derive(Debug, Default)]
pub struct CompilerInputCache {
    inputs: SlotMap<InputId, CompilerInput>,
}

impl CompilerInputCache {
    pub fn load_input_from_file(
        &mut self,
        path: &Path,
    ) -> Result<InputId, SleighCompilerError> {
        Ok(self.inputs.insert(CompilerInput::try_from(path)?))
    }

    pub fn input(
        &self,
        input_id: InputId,
    ) -> Result<&CompilerInput, SleighCompilerError> {
        self.inputs
            .get(input_id)
            .ok_or(SleighCompilerError::FileNotLoaded)
    }
}

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer,
    parser: SleighParser,
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
        let mut context = CompilerContext::default();
        let mut input_cache = CompilerInputCache::default();
        let input_id = input_cache.load_input_from_file(spec_path)?;
        let tokens = self.lexer.tokenize(&input_cache, &mut context, input_id)?;
        let ast = self
            .parser
            .parse(&input_cache, &mut context, input_id, tokens);
        // self.preprocessor_factory
        //     .build()
        //     .preprocess_tokenized(tokens)?;
        println!("{ast:?}");
        exit(0);
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
            parser: Default::default(),
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
