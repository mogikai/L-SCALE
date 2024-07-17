use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::SleighParser;
use crate::preprocessor::error::PreprocessorError;
use crate::preprocessor::SleighPreprocessorFactory;
use crate::spec::ProcessorSpecification;
use ariadne::{
    Cache,
    Source,
};
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::collections::HashMap;
use std::env::current_dir;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::fs;
use std::io::Error;
use std::path::{
    Path,
    PathBuf,
};
use std::process::exit;
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::{
    StringInterner,
    Symbol,
};
use thiserror::Error;

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod tokens;

new_key_type! { pub struct InputId; }

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
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

#[derive(Debug, Default, Clone)]
pub struct CompilerInputCache {
    loaded_files: HashMap<PathBuf, InputId>,
    input_paths: SlotMap<InputId, PathBuf>,
    input_sources: SecondaryMap<InputId, Source>,
    input_tokens: SecondaryMap<InputId, SleighTokens>,
}

impl Cache<InputId> for CompilerInputCache {
    type Storage = String;

    fn fetch(
        &mut self,
        id: &InputId,
    ) -> Result<&Source<Self::Storage>, Box<dyn Debug + '_>> {
        self.input_sources
            .get(*id)
            .ok_or(Box::new("Could not find input") as _)
    }

    fn display<'a>(
        &self,
        id: &'a InputId,
    ) -> Option<Box<dyn Display + 'a>> {
        self.input_paths
            .get(*id)
            .map(|input| Box::new(input.to_str().unwrap().to_string()) as _)
    }
}

#[derive(Debug, Error)]
pub enum InputCacheError {
    #[error("Could not read the provided sleigh file.")]
    ReadError(#[from] Error),
    #[error("The requested file was not loaded.")]
    FileNotLoaded,
    #[error("The input has not been tokenized yet and has no attached tokens.")]
    InputNotTokenized,
}

impl CompilerInputCache {
    pub fn load_input_from_file(
        &mut self,
        path: &Path,
    ) -> Result<InputId, InputCacheError> {
        let path = path.to_path_buf();
        if let Some(&id) = self.loaded_files.get(&path) {
            Ok(id)
        } else {
            let source = Source::from(fs::read_to_string(path.as_path())?);
            let id = self.input_paths.insert(path.clone());
            self.input_sources.insert(id, source);
            self.loaded_files.insert(path, id);
            Ok(id)
        }
    }

    pub fn input_path(
        &self,
        input_id: InputId,
    ) -> Result<&Path, InputCacheError> {
        self.input_paths
            .get(input_id)
            .ok_or(InputCacheError::FileNotLoaded)
            .map(PathBuf::as_path)
    }

    pub fn input_source(
        &self,
        input_id: InputId,
    ) -> Result<&Source, InputCacheError> {
        self.input_sources
            .get(input_id)
            .ok_or(InputCacheError::FileNotLoaded)
    }

    pub fn input_tokens(
        &self,
        input_id: InputId,
    ) -> Result<&SleighTokens, InputCacheError> {
        self.input_tokens
            .get(input_id)
            .ok_or(InputCacheError::InputNotTokenized)
    }

    pub fn attach_tokens(
        &mut self,
        input_id: InputId,
        tokens: SleighTokens,
    ) -> Result<(), InputCacheError> {
        self.input_tokens.insert(input_id, tokens);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer,
    parser: SleighParser,
}

#[derive(Debug, Clone)]
pub struct CompilerInput {
    /// The path from which the input was loaded.
    origin: PathBuf,
    /// The source data of the loaded input.
    source: Source,
    /// The tokens of the tokenized input, if it has already been tokenized.
    tokens: Option<SleighTokens>,
}

impl SleighCompiler {
    /// Compile the provided sleigh specification.
    pub fn compile(
        &mut self,
        spec_path: &Path,
    ) -> Result<ProcessorSpecification, SleighCompilerError> {
        let mut pp = self.preprocessor_factory.build();
        let mut context = CompilerContext::default();
        let mut input_cache = CompilerInputCache::default();
        let mut file = spec_path.to_path_buf();
        let tokens = loop {
            // Load the new file and check if it is already tokenized
            let input_id = input_cache.load_input_from_file(file.as_path())?;
            if input_cache.input_tokens(input_id).is_err() {
                // If the input was already tokenized, we can continue, otherwise tokenize now.
                let tokens = self
                    .lexer
                    .tokenize(&mut input_cache, &mut context, input_id)?;
                input_cache.attach_tokens(input_id, tokens)?;
            }
            // Parse the input that was loaded
            pp.parse_input(&mut input_cache, input_id)?;
            // Continue preprocessing
            match pp.preprocess(&mut input_cache, &mut context) {
                Ok(tokens) => break tokens,
                Err(PreprocessorError::LoadingRequired(f)) => file = f,
                Err(e) => Err(SleighCompilerError::from(e))?,
            }
        };
        let ast = self.parser.parse(&mut input_cache, &mut context, tokens);
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
        SleighCompiler::new(self.preprocessor_factory.unwrap_or_default())
    }
}
