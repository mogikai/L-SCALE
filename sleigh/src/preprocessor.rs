use std::collections::HashMap;
use std::env::current_dir;
use std::fmt::{
    Debug,
    Formatter,
};
use std::iter::empty;
use std::mem;
use std::path::{
    Component,
    PathBuf,
};

use ariadne::{
    Label,
    Report,
    ReportKind,
};
use chumsky::prelude::end;
use chumsky::span::Span;
use chumsky::{
    select,
    Parser,
};
use tracing::trace;

use blocks::{
    BinaryOp,
    ComparisonArgument,
    EqualityCheck,
    Expression,
    PreprocessorBlock,
};

use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
    SleighLexer,
    SleighTokens,
};
use crate::compiler::parser::{
    inline_whitespace_padded,
    maybe_parenthesized,
    parenthesized,
};
use crate::compiler::tokens::{
    IncludedTokens,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use crate::compiler::{
    CompilerContext,
    CompilerInputCache,
    InputId,
    StringId,
};
use crate::preprocessor::error::PreprocessorError;

mod blocks;
pub mod error;
mod parser;

pub struct SleighPreprocessor {
    /// Associate identifiers with strings. An identifier is simply a string with a specific identifier format.
    definitions: HashMap<StringId, Spanned<StringId>>,
    /// The output token stream that is constructed from multiple input token streams
    token_streams: HashMap<InputId, SleighTokens>,
    origin_input_id: Option<InputId>,
    blocks: Box<dyn Iterator<Item = Spanned<PreprocessorBlock>>>,
    /// The working directory of the preprocessor. Only files in this directory or its descendants may be included.
    working_dir: PathBuf,
}

impl Debug for SleighPreprocessor {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("SleighPreprocessor2")
            .field("definitions", &self.definitions)
            .field("token_streams", &self.token_streams)
            .field("working_dir", &self.working_dir)
            .finish()
    }
}

impl SleighPreprocessor {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            definitions: Default::default(),
            token_streams: Default::default(),
            origin_input_id: None,
            blocks: Box::new(empty()),
            working_dir,
        }
    }

    pub fn parse_input(
        &mut self,
        input_cache: &mut CompilerInputCache,
        input_id: InputId,
    ) -> Result<(), PreprocessorError> {
        if self.origin_input_id.is_none() {
            // The first input that is parsed is the origin file.
            self.origin_input_id = Some(input_id);
        }
        // Insert an empty vec for future use
        self.token_streams.insert(input_id, Default::default());
        let new_iter = self.parse_macros(input_cache, input_id)?.into_iter();
        let remaining_blocks = mem::replace(&mut self.blocks, Box::new(empty()));
        self.blocks = Box::new(new_iter.chain(remaining_blocks));
        Ok(())
    }

    fn prepend_blocks(
        &mut self,
        blocks: Vec<Spanned<PreprocessorBlock>>,
    ) {
        let blocks = blocks.into_iter();
        let remaining_blocks = mem::replace(&mut self.blocks, Box::new(empty()));
        self.blocks = Box::new(blocks.chain(remaining_blocks));
    }

    pub fn preprocess(
        &mut self,
        input_cache: &mut CompilerInputCache,
        context: &mut CompilerContext,
    ) -> Result<SleighTokens, PreprocessorError> {
        loop {
            let Some((block, span)) = self.blocks.next() else {
                // We finished processing all blocks. We can now construct the final token stream
                break Ok(self.finalize_tokens()?);
            };
            match block {
                PreprocessorBlock::Define { name, value } => self.process_define(name, value),
                PreprocessorBlock::Undefine((name, _)) => self.process_undefine(&name),
                PreprocessorBlock::Include((file, _)) => self.process_include(input_cache, context, span, file)?,
                PreprocessorBlock::IfDef {
                    ident,
                    if_defined,
                    if_undefined,
                } => self.process_ifdef(&ident, if_defined, if_undefined),
                PreprocessorBlock::If {
                    cond,
                    if_true,
                    if_false,
                } => self.process_if(context, cond, if_true, if_false)?,
                PreprocessorBlock::PCode((tokens, _)) => self.process_pcode(tokens),
                PreprocessorBlock::Expansion((ident, _)) => self.process_expansion(context, ident)?,
            }
        }
    }

    fn process_expansion(
        &mut self,
        context: &mut CompilerContext,
        ident: StringId,
    ) -> Result<(), PreprocessorError> {
        let resolved_ident = context
            .string(ident)
            .expect("The string must have been interned.")
            .to_string();
        let (value, span) = self
            .definitions
            .get(&ident)
            .ok_or(PreprocessorError::UndefinedIdentifier(resolved_ident))?;
        let resolved_value = context
            .string(*value)
            .expect("The string must have been interned.")
            .to_string();
        // The offset is start + 1, since the first character of a string is ", which is ignored.
        let offset = span.start() + 1;
        let result = SleighLexer::tokenize_string(resolved_value.as_str(), context, span.context(), offset);
        let out = result.output().unwrap().as_slice();
        let span = out[0].1.clone();
        let tokens = result
            .into_result()
            .map_err(|e| PreprocessorError::ParseError("Failed macro expansion".to_string()))?;
        // Append the tokens to the stream associated with the input they came from
        self.token_streams
            .entry(tokens.input_id())
            .or_default()
            .extend(tokens);
        Ok(())
    }

    fn process_pcode(
        &mut self,
        tokens: SleighTokens,
    ) {
        // Append the tokens to the stream associated with the input they came from
        self.token_streams
            .entry(tokens.input_id())
            .or_default()
            .extend(tokens);
    }

    fn process_if(
        &mut self,
        context: &mut CompilerContext,
        cond: Spanned<Expression>,
        if_true: Spanned<Vec<Spanned<PreprocessorBlock>>>,
        if_false: Option<Spanned<Vec<Spanned<PreprocessorBlock>>>>,
    ) -> Result<(), PreprocessorError> {
        trace!("Evaluating condition {cond:?}");
        if self.evaluate_expression(context, cond)? {
            trace!("Evaluated to true");
            self.prepend_blocks(if_true.0);
        } else {
            trace!("Evaluated to false");
            match if_false {
                None => {}
                Some(blocks) => self.prepend_blocks(blocks.0),
            }
        }
        Ok(())
    }

    fn process_ifdef(
        &mut self,
        ident: &Spanned<StringId>,
        if_defined: Option<Spanned<Vec<Spanned<PreprocessorBlock>>>>,
        if_undefined: Option<Spanned<Vec<Spanned<PreprocessorBlock>>>>,
    ) {
        trace!("Processing ifdef {ident:?}");
        if self.definitions.contains_key(&ident.0) {
            trace!("{:?} is defined", ident.0);
            match if_defined {
                None => {}
                Some(blocks) => self.prepend_blocks(blocks.0),
            }
        } else {
            trace!("{ident:?} is undefined");
            match if_undefined {
                None => {}
                Some(blocks) => self.prepend_blocks(blocks.0),
            };
        }
    }

    fn process_undefine(
        &mut self,
        name: &StringId,
    ) {
        trace!("Removed definition for {name:?}");
        self.definitions.remove(name);
    }

    fn process_define(
        &mut self,
        name: Spanned<StringId>,
        value: Spanned<StringId>,
    ) {
        trace!("Defining {:?} as {:?}", name.0, value.0);
        self.definitions.insert(name.0, value);
    }

    fn finalize_tokens(&mut self) -> Result<SleighTokens, PreprocessorError> {
        let mut token_stream = self
            .token_streams
            .remove(
                &self
                    .origin_input_id
                    .ok_or(PreprocessorError::NoInputProcessed)?,
            )
            .expect("If origin_input_id is set we must have processed the corresponding input.");
        token_stream.iter_mut().for_each(|token| {
            if let SleighToken::IncludedTokens(IncludedTokens::PendingProcessing(input_id)) = token.0 {
                token.0 =
                    SleighToken::IncludedTokens(IncludedTokens::Loaded(self.token_streams.remove(&input_id).unwrap()));
            }
        });
        Ok(token_stream)
    }

    fn process_include(
        &mut self,
        input_cache: &mut CompilerInputCache,
        context: &mut CompilerContext,
        span: InputSpan,
        file: StringId,
    ) -> Result<(), PreprocessorError> {
        let file = PathBuf::from(
            context
                .string(file)
                .expect("String has to have been interned"),
        );
        trace!("Including file {file:?}");
        if file.is_absolute() {
            // TODO: Error reporting
            Err(PreprocessorError::AbsolutePath)?;
        }
        let mut resolved_file = self.working_dir.clone();
        for part in file.components() {
            if let Component::Normal(path) = part {
                resolved_file.push(path);
            }
        }
        let input_id = input_cache.load_input_from_file(resolved_file.as_path())?;
        self.token_streams
            .get_mut(&span.context())
            .expect("The input that issued the include must exist.")
            .extend([(SleighToken::IncludedTokens(IncludedTokens::PendingProcessing(input_id)), span)]);
        Err(PreprocessorError::LoadingRequired(input_id))
    }

    fn evaluate_expression(
        &self,
        context: &CompilerContext,
        expression: Spanned<Expression>,
    ) -> Result<bool, PreprocessorError> {
        Ok(match expression.0 {
            Expression::BinaryOp(a, op, b) => match op.0 {
                BinaryOp::And => self.evaluate_expression(context, *a)? && self.evaluate_expression(context, *b)?,
                BinaryOp::Or => self.evaluate_expression(context, *a)? || self.evaluate_expression(context, *b)?,
                BinaryOp::Xor => self.evaluate_expression(context, *a)? ^ self.evaluate_expression(context, *b)?,
            },
            Expression::Defined(ident) => self.definitions.get(&ident).map(|_| true).unwrap_or(false),
            Expression::EqualityCheck(check) => match check {
                EqualityCheck::Eq(a, b) => {
                    self.resolve_comparison_argument(context, &a)? == self.resolve_comparison_argument(context, &b)?
                }
                EqualityCheck::Ne(a, b) => {
                    self.resolve_comparison_argument(context, &a)? != self.resolve_comparison_argument(context, &b)?
                }
            },
        })
    }

    fn resolve_comparison_argument(
        &self,
        context: &CompilerContext,
        arg: &Spanned<ComparisonArgument>,
    ) -> Result<StringId, PreprocessorError> {
        Ok(match arg.0 {
            ComparisonArgument::Identifier(ident) => {
                self.definitions
                    .get(&ident)
                    .ok_or(PreprocessorError::UndefinedIdentifier(context.string(ident).unwrap().to_string()))?
                    .0
            }
            ComparisonArgument::Literal(literal) => literal,
        })
    }

    fn parse_macros(
        &self,
        mut input_cache: &mut CompilerInputCache,
        input_id: InputId,
    ) -> Result<Vec<Spanned<PreprocessorBlock>>, PreprocessorError> {
        let tokens = input_cache.input_tokens(input_id)?;
        let parsed = parser::parser().parse(tokens.spanned());
        let mut diagnostic = vec![];
        if parsed.has_errors() {
            // TODO: Can we be smarter here and not copy the errors? Currently this is necessary to be able to borrow
            //       the input cache again.
            let errors = parsed
                .into_errors()
                .into_iter()
                .map(|e| e.into_owned())
                .collect::<Vec<_>>();
            for err in errors {
                let error_span = err.span();
                let report = Report::build(ReportKind::Error, error_span.context(), error_span.start())
                    .with_message("Preprocessor Error")
                    .with_label(Label::new(error_span.clone()).with_message(err.reason().to_string()))
                    .finish();
                report.write(&mut input_cache, &mut diagnostic).unwrap();
            }
            Err(PreprocessorError::ParseError(String::from_utf8(diagnostic).unwrap()))
        } else {
            Ok(parsed.into_output().unwrap())
        }
    }
}

#[derive(Default, Debug)]
pub struct SleighPreprocessorFactory {
    working_dir: Option<PathBuf>,
}

impl SleighPreprocessorFactory {
    pub fn new() -> Self {
        Self { working_dir: None }
    }

    pub fn working_dir(
        mut self,
        dir: PathBuf,
    ) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn build(&self) -> SleighPreprocessor {
        SleighPreprocessor::new(
            self.working_dir
                .clone()
                .unwrap_or_else(|| current_dir().unwrap()),
        )
    }
}
