use std::collections::HashMap;
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

use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::with::SASTWith;
use crate::compiler::ast::{
    SleighAST,
    Spanned,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::SleighParser;
use crate::preprocessor::error::PreprocessorError;
use crate::preprocessor::{
    SleighPreprocessor,
    SleighPreprocessorFactory,
};
use crate::spec::ProcessorSpecification;
use ariadne::{
    Cache,
    Source,
};
use inputcache::CompilerInputCache;
use slotmap::{
    new_key_type,
    SecondaryMap,
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
use thiserror::Error;
use tracing::{
    debug,
    trace,
};

pub mod ast;
pub mod builder;
pub mod error;
pub mod inputcache;
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

pub(crate) trait CompilerContext {
    /// Intern the provided value that can be converted to a string.
    /// If the value already exists, a key to that value is returned.
    /// Otherwise, a new entry is created and the key to that is returned.
    fn intern<T: AsRef<str>>(
        &mut self,
        string: &T,
    ) -> StringId;

    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str>;
}

impl CompilerContext for ProcessorSpecification {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: &T,
    ) -> StringId {
        StringId(self.interner.get_or_intern(string).to_usize())
    }

    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.interner
            .resolve(<<StringBackend as Backend>::Symbol as Symbol>::try_from_usize(string_id.0)?)
    }
}

pub type SleighCompilerResult<T = ()> = Result<T, SleighCompilerError>;

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer<ProcessorSpecification>,
    parser: SleighParser,
    input_cache: CompilerInputCache,
    specification: ProcessorSpecification,
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
        let tokens = self.load_and_preprocess(spec_path)?;
        let ast = self
            .parser
            .parse(&mut self.input_cache, &mut self.specification, tokens)?;
        self.build(ast)?;
        Ok(self.specification)
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
        let mut input_id = self.input_cache.load_input_from_file(file.as_path())?;
        debug!("Loading initial file {file:?} as {input_id:?}");
        let tokens = loop {
            // Check if requested input is already tokenized
            if self.input_cache.input_tokens(input_id).is_err() {
                // If the input was already tokenized, we can continue, otherwise tokenize now.
                let tokens = self
                    .lexer
                    .tokenize(&mut self.input_cache, &mut self.specification, input_id)?;
                debug!("Tokenized input {input_id:?}");
                self.input_cache.attach_tokens(input_id, tokens)?;
            }
            // Parse the input that was loaded
            preprocessor.parse_input(&mut self.input_cache, input_id)?;
            // Continue preprocessing
            match preprocessor.preprocess(&mut self.input_cache, &mut self.specification) {
                Ok(tokens) => break tokens,
                Err(PreprocessorError::LoadingRequired(id)) => input_id = id,
                Err(e) => Err(SleighCompilerError::from(e))?,
            }
        };
        Ok(tokens)
    }

    fn build(
        &mut self,
        ast: SleighAST,
    ) -> SleighCompilerResult {
        debug!("Building the processor specification from the AST.");
        debug!("The AST has {} toplevel items", ast.items.len());
        self.process_items(ast.items)
    }

    fn process_items(
        &mut self,
        items: Vec<SASTItem>,
    ) -> SleighCompilerResult {
        for item in items {
            match item {
                SASTItem::Attach(attach) => {}
                SASTItem::Constructor(constructor) => {}
                SASTItem::Define(_) => {}
                SASTItem::Macro(pcode_macro) => self.process_pcode_macro(pcode_macro)?,
                SASTItem::With(with) => self.process_with(with)?,
                SASTItem::Error(_) => {}
            }
        }
        Ok(())
    }

    fn process_with(
        &mut self,
        (with, span): Spanned<SASTWith>,
    ) -> SleighCompilerResult {
        trace!("Processing with statement at {span:?}.");
        self.process_items(with.items)?;
        Ok(())
    }

    fn process_pcode_macro(
        &mut self,
        (pcode_macro, span): Spanned<SASTPCodeMacro>,
    ) -> SleighCompilerResult {
        trace!("Processing pcode macro at {span:?}");
        self.specification.resolve_ident_to_id(pcode_macro.ident);
        // self.specification.mir_macros
        Ok(())
    }
}

impl SleighCompiler {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
            parser: Default::default(),
            input_cache: Default::default(),
            specification: Default::default(),
        }
    }
}
