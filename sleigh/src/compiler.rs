use crate::compiler::error::{
    InternalError,
    InternalErrorType,
    SleighCompilerError,
};
use crate::spec::ProcessorSpecification;
use crate::spec::builder::ProcessorSpecificationBuilder;
use ariadne::Source;
use mogi_sleigh_parser::inputcache::CompilerInputCache;
use mogi_sleigh_parser::lexer::{
    SleighLexer,
    SleighTokens,
};
use mogi_sleigh_parser::parser::SleighParser;
use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;
use mogi_sleigh_parser::preprocessor::error::PreprocessorError;
use std::fmt::Debug;
use std::path::{
    Path,
    PathBuf,
};
use tracing::{
    debug,
    info,
};

pub mod build;
pub mod builder;
pub mod error;

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
            .parse(&mut self.input_cache, &mut self.spec_builder.interner_mut(), tokens)
            .map_err(|_| -> SleighCompilerError { todo!("Correctly map errors") })?;
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
                    .tokenize(&mut self.input_cache, &mut self.spec_builder.interner_mut(), input_id)
                    .map_err(|_| -> SleighCompilerError { todo!("Correctly map errors") })?;
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
            match preprocessor.preprocess(&mut self.input_cache, &mut self.spec_builder.interner_mut()) {
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
