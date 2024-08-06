use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::path::{
    Path,
    PathBuf,
};

use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
    Source,
};
use chumsky::container::Container;
use chumsky::span::Span;
use slotmap::{
    new_key_type,
    SlotMap,
};
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::Symbol;
use tracing::{
    debug,
    trace,
};

use inputcache::CompilerInputCache;

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
use crate::preprocessor::SleighPreprocessorFactory;
use crate::spec::moir::MOIRMacro;
use crate::spec::{
    ProcessorEndianness,
    ProcessorSpecification,
    ProcessorSpecificationBuilder,
    ProcessorSpecificationError,
    StringId,
};

pub mod ast;
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
        string: &T,
    ) -> StringId;

    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str>;
}

pub type SleighCompilerResult<T = ()> = Result<T, SleighCompilerError>;

#[derive(Debug)]
pub struct SleighCompiler {
    preprocessor_factory: SleighPreprocessorFactory,
    lexer: SleighLexer<ProcessorSpecificationBuilder>,
    parser: SleighParser,
    input_cache: CompilerInputCache,
    specification: ProcessorSpecificationBuilder,
    errors: Vec<Box<Report<'static, InputSpan>>>,
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
        if self.errors.is_empty() {
            Ok(self.specification.build()?)
        } else {
            let mut error_description = vec![];
            let error_count = self.errors.len();
            for report in self.errors {
                report
                    .write(&mut self.input_cache, &mut error_description)
                    .expect("There is no way this may fail.");
            }
            error_description.extend(
                format!(
                    "───────────────────────────────\nFound a total of {} errors.\n───────────────────────────────",
                    error_count
                )
                .as_bytes(),
            );
            Err(SleighCompilerError::SemanticError(
                String::from_utf8(error_description).expect("We only write utf8 strings."),
            ))
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
        constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        trace!("Processing constructor statement");
        let (constructor, span) = constructor.into_tuple();
        let table = match constructor.table.lift_option() {
            None => self.specification.modify_root_table()?,
            Some(id) => self.specification.modify_table(id)?,
        };
        // let table = match lift_option(constructor.table) {
        //     None => MOIRIdentId::default(), // TODO: Root table
        //     Some(id) => self.specification.reserve_global_ident(id)?,
        // };
        Ok(())
    }

    fn process_define(
        &mut self,
        define: Spanned<SASTDefine>,
    ) -> SleighCompilerResult {
        trace!("Processing define statement");
        match define.into_value() {
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
        endianness: Spanned<SASTEndianness>,
    ) -> SleighCompilerResult {
        let result = self.specification.define_endianness(endianness.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_alignment(
        &mut self,
        alignment: Spanned<SASTAlignment>,
    ) -> SleighCompilerResult {
        let result = self.specification.define_alignment(alignment.transmute());
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    fn process_with(
        &mut self,
        with: Spanned<SASTWith>,
    ) -> SleighCompilerResult {
        trace!("Processing with statement at {:?}.", with.span());
        self.process_items(with.into_value().items)?;
        Ok(())
    }

    fn process_pcode_macro(
        &mut self,
        pcode_macro: Spanned<SASTPCodeMacro>,
    ) -> SleighCompilerResult {
        trace!("Processing pcode macro at {:?}", pcode_macro.span());
        let result = self.specification.define_macro(
            pcode_macro.into_value().ident.transmute(),
            MOIRMacro {
                arguments: vec![],
                code: vec![],
            },
        );
        self.report_error_or_unwrap(result)?;
        Ok(())
    }

    /// Reports an error if the error type is `Report` and returns `None`.
    /// If the result is Ok, then this function returns `Ok(Some(value))`
    /// Otherwise, this function errors as well.
    fn report_error_or_unwrap<T>(
        &mut self,
        result: Result<T, ProcessorSpecificationError>,
    ) -> SleighCompilerResult<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                if let ProcessorSpecificationError::ErrorReport(report) = err {
                    self.errors.push(report);
                    Ok(None)
                } else {
                    Err(err)?
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
            specification: Default::default(),
            errors: vec![],
        }
    }
}
