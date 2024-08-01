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

use ariadne::{
    Cache,
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::span::Span;
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

use inputcache::CompilerInputCache;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::define::{
    SASTAlignment,
    SASTDefine,
    SASTEndianness,
};
use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::pcode_macro::SASTPCodeMacro;
use crate::compiler::ast::with::SASTWith;
use crate::compiler::ast::{
    SleighAST,
    Spanned,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    InputSpan,
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::SleighParser;
use crate::preprocessor::error::PreprocessorError;
use crate::preprocessor::{
    SleighPreprocessor,
    SleighPreprocessorFactory,
};
use crate::spec::moir::MOIRMacro;
use crate::spec::{
    MOIRIdent,
    ProcessorSpecificationBuilder,
    ProcessorSpecificationError,
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

impl CompilerContext for ProcessorSpecificationBuilder {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: &T,
    ) -> StringId {
        StringId(self.interner.get_or_intern(string).to_usize())
    }

    /// Resolve a string id to the corresponding interned string.
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
pub struct SleighCompiler<'a> {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer<ProcessorSpecificationBuilder>,
    parser: SleighParser,
    input_cache: CompilerInputCache,
    specification: ProcessorSpecificationBuilder,
    errors: Vec<Report<'a, InputSpan>>,
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

impl SleighCompiler<'_> {
    /// Compile the provided sleigh specification, consuming the compiler instance.
    pub fn compile(
        mut self,
        spec_path: &Path,
    ) -> SleighCompilerResult<ProcessorSpecificationBuilder> {
        let tokens = self.load_and_preprocess(spec_path)?;
        let ast = self
            .parser
            .parse(&mut self.input_cache, &mut self.specification, tokens)?;
        self.build(ast)?;
        if self.errors.is_empty() {
            Ok(self.specification)
        } else {
            let mut error_description = vec![];
            let error_count = self.errors.len();
            for report in self.errors {
                report
                    .write(&mut self.input_cache, &mut error_description)
                    .unwrap();
            }
            error_description.extend(
                format!(
                    "───────────────────────────────\nFound a total of {} errors.\n───────────────────────────────",
                    error_count
                )
                .as_bytes(),
            );
            Err(SleighCompilerError::SemanticError(String::from_utf8(error_description).unwrap()))
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
                SASTItem::Constructor(constructor) => self.process_constructor(constructor)?,
                SASTItem::Define(define) => self.process_define(define)?,
                SASTItem::Macro(pcode_macro) => self.process_pcode_macro(pcode_macro)?,
                SASTItem::With(with) => self.process_with(with)?,
                SASTItem::Error(_) => {}
            }
        }
        Ok(())
    }

    fn process_constructor(
        &mut self,
        (constructor, span): Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        trace!("Processing constructor statement");
        // let table = match lift_option(constructor.table) {
        //     None => MOIRIdentId::default(), // TODO: Root table
        //     Some(id) => self.specification.reserve_global_ident(id)?,
        // };
        Ok(())
    }

    fn process_define(
        &mut self,
        (define, span): Spanned<SASTDefine>,
    ) -> SleighCompilerResult {
        trace!("Processing define statement");
        match define {
            SASTDefine::Endianness(endianness) => self.process_endianness(endianness),
            SASTDefine::Alignment(alignment) => self.process_alignment(alignment),
            SASTDefine::AddressSpace(_) => Ok(()),
            SASTDefine::Register(_) => Ok(()),
            SASTDefine::BitrangeRegister(_) => Ok(()),
            SASTDefine::UserPcodeOp(_) => Ok(()),
            SASTDefine::Token(_) => Ok(()),
            SASTDefine::Context(_) => Ok(()),
        }
    }

    fn process_endianness(
        &mut self,
        (endianness, span): Spanned<SASTEndianness>,
    ) -> SleighCompilerResult {
        if let Some((_, old_span)) = &self.specification.endianness {
            self.errors
                .push(self.generate_endianness_already_specified_report(old_span.clone(), span));
        } else {
            self.specification.endianness = Some((endianness.into(), span));
        };
        Ok(())
    }

    fn process_alignment(
        &mut self,
        (alignment, span): Spanned<SASTAlignment>,
    ) -> SleighCompilerResult {
        if let Some((_, old_span)) = &self.specification.alignment {
            self.errors
                .push(self.generate_alignment_already_specified_report(old_span.clone(), span))
        } else {
            self.specification.alignment = Some((alignment, span))
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
        if let Err(report) = self.specification.define_macro(
            (pcode_macro.ident.0 .0, pcode_macro.ident.1),
            MOIRMacro {
                arguments: vec![],
                code: vec![],
            },
        ) {
            // If defining the macro failed, we simply report the error and continue.
            self.errors.push(report);
        }
        Ok(())
    }

    fn generate_endianness_already_specified_report<'a>(
        &self,
        old: InputSpan,
        new: InputSpan,
    ) -> Report<'a, InputSpan> {
        let mut colors = ColorGenerator::new();
        let color_a = colors.next();
        let report = Report::build(ReportKind::Error, new.context(), new.start())
            .with_message("Endianness already specified")
            .with_label(
                Label::new(new)
                    .with_message("Attempting to redefine endianness when it is already defined.")
                    .with_color(color_a),
            )
            .with_label(
                Label::new(old)
                    .with_message(format!(
                        "The endianness is already defined here as {}.",
                        self.specification
                            .endianness
                            .as_ref()
                            .unwrap()
                            .0
                            .fg(color_a)
                    ))
                    .with_color(color_a),
            )
            .finish();
        report
    }

    fn generate_alignment_already_specified_report<'a>(
        &self,
        old: InputSpan,
        new: InputSpan,
    ) -> Report<'a, InputSpan> {
        let mut colors = ColorGenerator::new();
        let color_a = colors.next();
        let report = Report::build(ReportKind::Error, new.context(), new.start())
            .with_message("Alignment already specified")
            .with_label(
                Label::new(new)
                    .with_message("Attempting to redefine alignment when it is already defined.")
                    .with_color(color_a),
            )
            .with_label(
                Label::new(old)
                    .with_message(format!(
                        "The alignment is already defined here as {}.",
                        self.specification.alignment.as_ref().unwrap().0.fg(color_a)
                    ))
                    .with_color(color_a),
            )
            .finish();
        report
    }
}

fn lift_option<T>((x, span): Spanned<Option<T>>) -> Option<Spanned<T>> {
    x.map(|x| (x, span))
}

impl SleighCompiler<'_> {
    fn new(preprocessor_factory: SleighPreprocessorFactory) -> Self {
        Self {
            preprocessor_factory,
            lexer: Default::default(),
            parser: Default::default(),
            input_cache: Default::default(),
            specification: Default::default(),
            errors: vec![],
        }
    }
}
