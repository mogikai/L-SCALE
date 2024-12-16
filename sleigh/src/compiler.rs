use ariadne::Source;
use slotmap::new_key_type;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::path::{
    Path,
    PathBuf,
};
use tracing::{
    debug,
    info,
};

use crate::compiler::error::{
    InternalError,
    InternalErrorType,
    SleighCompilerError,
};
use crate::compiler::lexer::{
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::SleighParser;
use crate::preprocessor::SleighPreprocessorFactory;
use crate::preprocessor::error::PreprocessorError;
use crate::spec::ProcessorSpecification;
use crate::spec::builder::{
    ProcessorSpecificationBuilder,
    StringId,
};
use inputcache::CompilerInputCache;

pub mod ast;
pub mod build;
pub mod builder;
pub mod error;
pub mod inputcache;
pub mod lexer;
pub mod parser;
pub mod tokens;

new_key_type! { pub struct InputId; }

impl Display for InputId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

pub(crate) trait CompilerContext {
    /// Intern the provided value that can be converted to a string.
    /// If the value already exists, a key to that value is returned.
    /// Otherwise, a new entry is created and the key to that is returned.
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId;

    /// Get the id for an interned string if it exists.
    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId>;

    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str>;
}

pub type InternalCompilerResult<T = ()> = Result<T, InternalError>;

pub type SleighCompilerResult<T = ()> = Result<T, SleighCompilerError>;

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer,
    parser: SleighParser,
    input_cache: CompilerInputCache,
    spec_builder: ProcessorSpecificationBuilder,
    errors: Vec<InternalError>,
    stop_on_first_error: bool,
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
    /// Compile the provided sleigh specification, consuming the compiler instance.
    pub fn compile(
        mut self,
        spec_path: &Path,
    ) -> SleighCompilerResult<ProcessorSpecification> {
        info!("Loading and preprocessing {spec_path:?} and the files it includes.");
        let tokens = self.load_and_preprocess(spec_path)?;
        info!("Parsing the abstract syntax tree.");
        let ast = self
            .parser
            .parse(&mut self.input_cache, &mut self.spec_builder, tokens)?;
        info!("Building the specification from the AST.");
        let result = self.build(ast);
        // result should not be an error except StopOnFirstError, if result is an error
        // we failed to handle this error properly.
        if let Err(e) = result {
            if !matches!(e.error, InternalErrorType::StopOnFirstError) {
                return Err(SleighCompilerError::ErrorHandlingFailure);
            }
        }
        if self.errors.is_empty() {
            Ok(self
                .spec_builder
                .build()
                .map_err(|e| SleighCompilerError::SemanticError(e.create_string(&mut self.input_cache)))?)
        } else {
            let mut error_description = String::new();
            let error_count = self.errors.len();
            for error in self.errors {
                error_description += &error.create_string(&mut self.input_cache)
            }
            error_description += &format!(
                "───────────────────────────────\nFound a total of {} errors.\n───────────────────────────────",
                error_count
            );
            Err(SleighCompilerError::SemanticError(error_description))
        }
    }

    /// Preprocess the file by loading it, then tokenizing it, and preprocessing it afterward.
    /// The actual preprocessing expands macros and also includes files included with the `@include`
    /// macro. These included files are also tokenized and then preprocessed recursively.
    fn load_and_preprocess(
        &mut self,
        spec_path: &Path,
    ) -> SleighCompilerResult<SleighTokens> {
        let mut preprocessor = self.preprocessor_factory.build();
        let file = spec_path.to_path_buf();
        let mut input_id = self
            .input_cache
            .load_input_from_file(file.as_path())
            .map_err(|e| {
                SleighCompilerError::LexerError(InternalError::from(e).create_string(&mut self.input_cache))
            })?;
        debug!("Loading initial file {file:?} as {input_id:?}");
        let tokens = loop {
            // Check if requested input is already tokenized
            if self.input_cache.input_tokens(input_id).is_err() {
                // If the input was already tokenized, we can continue, otherwise tokenize now.
                let tokens = self
                    .lexer
                    .tokenize(&mut self.input_cache, &mut self.spec_builder, input_id)?;
                debug!("Tokenized input {input_id:?}");
                self.input_cache
                    .attach_tokens(input_id, tokens)
                    .map_err(|e| {
                        SleighCompilerError::LexerError(InternalError::from(e).create_string(&mut self.input_cache))
                    })?;
            }
            // Parse the input that was loaded
            preprocessor
                .parse_input(&mut self.input_cache, input_id)
                .map_err(|e| {
                    SleighCompilerError::LexerError(InternalError::from(e).create_string(&mut self.input_cache))
                })?;
            // Continue preprocessing
            match preprocessor.preprocess(&mut self.input_cache, &mut self.spec_builder) {
                Ok(tokens) => break tokens,
                Err(PreprocessorError::LoadingRequired(id)) => input_id = id,
                Err(e) => {
                    Err(SleighCompilerError::LexerError(InternalError::from(e).create_string(&mut self.input_cache)))?
                }
            }
        };
        Ok(tokens)
    }

    /// Reports an error if the error type is `Report` and returns `None`.
    /// If the result is Ok, then this function returns `Ok(Some(value))`
    /// Otherwise, this function errors as well.
    fn report_error_or_unwrap<T>(
        &mut self,
        result: Result<T, InternalError>,
    ) -> InternalCompilerResult<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                self.errors.push(err);
                if self.stop_on_first_error {
                    Err(InternalErrorType::StopOnFirstError.into())
                } else {
                    Ok(None)
                }
            }
        }
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
            parser: Default::default(),
            input_cache: Default::default(),
            spec_builder: Default::default(),
            errors: vec![],
            stop_on_first_error: false,
        }
    }
}
