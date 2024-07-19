use std::{
    fs,
    mem,
};
use std::collections::HashMap;
use std::env::current_dir;
use std::fmt::{
    Debug,
    Formatter,
};
use std::iter::empty;
use std::path::{
    Component,
    PathBuf,
};

use ariadne::{
    Label,
    Report,
    ReportKind,
};
use chumsky::{
    extra,
    IterParser,
    Parser,
    select,
};
use chumsky::error::Rich;
use chumsky::input::MapExtra;
use chumsky::prelude::{
    any,
    end,
};
use chumsky::primitive::just;
use chumsky::recursive::Recursive;
use chumsky::span::Span;
use tracing::trace;

use crate::compiler::{
    CompilerContext,
    CompilerInputCache,
    InputId,
    StringId,
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
    whitespace_padded,
};
use crate::compiler::tokens::{
    IncludedTokens,
    SleighKeyword,
    SleighOperation,
    SleighToken,
};
use crate::preprocessor::error::PreprocessorError;

pub mod error;

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
                        token.0 = SleighToken::IncludedTokens(IncludedTokens::Loaded(self.token_streams.remove(&input_id).unwrap()));
                    }
                });
                break Ok(token_stream);
            };
            match block {
                PreprocessorBlock::Define { name, value } => {
                    trace!("Defining {:?} as {:?}", name.0, value.0);
                    self.definitions.insert(name.0, value);
                }
                PreprocessorBlock::Undefine((name, _)) => {
                    trace!("Removed definition for {name:?}");
                    self.definitions.remove(&name);
                }
                PreprocessorBlock::Include((file, _)) => self.process_include(input_cache, context, span, file)?,
                PreprocessorBlock::IfDef {
                    ident,
                    if_defined,
                    if_undefined,
                } => {
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
                PreprocessorBlock::If {
                    cond,
                    if_true,
                    if_false,
                } => {
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
                }
                PreprocessorBlock::PCode((tokens, _)) => {
                    // Append the tokens to the stream associated with the input they came from
                    self.token_streams
                        .entry(tokens.input_id())
                        .or_default()
                        .extend(tokens);
                }
                PreprocessorBlock::Expansion((ident, _)) => {
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
                }
            }
        }
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
        let parsed = parser().parse(tokens.spanned());
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

type SleighPreprocessorExtra<'tokens> = extra::Full<Rich<'tokens, SleighToken, InputSpan>, CompilerContext, ()>;

fn parser<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Vec<Spanned<PreprocessorBlock>>, SleighPreprocessorExtra<'tokens>> {
    let mut blocks = Recursive::declare();

    let ident = ident();
    let string = select! { SleighToken::String(string) => string }.labelled("string");
    let macro_token = select! { SleighToken::MacroBegin => (), SleighToken::MacroExpansion(_) => ()};

    // Skip anything that is not a macro token and collect it into a PCode block.
    let pcode = any()
        .and_is(macro_token.not())
        .map_with(|token, extra| (token, extra.span()))
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .map_with(|tokens, extra| PreprocessorBlock::PCode((SleighTokens::from(tokens), extra.span())));

    let expansion =
        select! {SleighToken::MacroExpansion(ident) = extra => PreprocessorBlock::Expansion((ident, extra.span())) };

    let define = pp_macro(
        SleighKeyword::Define,
        inline_whitespace_padded(ident.map_with(spanned))
            .then(inline_whitespace_padded(string.map_with(spanned)))
            .map(|(name, value)| PreprocessorBlock::Define { name, value }),
    );

    let undefine = pp_macro(
        SleighKeyword::Undefine,
        inline_whitespace_padded(ident.map_with(spanned)).map(PreprocessorBlock::Undefine),
    );

    let include = pp_macro(
        SleighKeyword::Include,
        inline_whitespace_padded(string.map_with(spanned)).map(PreprocessorBlock::Include),
    );

    let endif = pp_macro_kw(SleighKeyword::EndIf);

    let r#else = pp_macro_kw(SleighKeyword::Else)
        .ignore_then(blocks.clone().map_with(spanned))
        .then_ignore(endif.clone());

    let ifdef_stencil = |keyword| {
        pp_macro(keyword, inline_whitespace_padded(ident.map_with(spanned)))
            .then(blocks.clone().map_with(spanned).map(Some))
            .then(r#else.clone().map(Some).or(endif.clone().to(None)))
    };

    let ifdef =
        ifdef_stencil(SleighKeyword::IfDef).map(|((ident, if_defined), if_undefined)| PreprocessorBlock::IfDef {
            ident,
            if_defined,
            if_undefined,
        });

    let ifndef =
        ifdef_stencil(SleighKeyword::IfNdef).map(|((ident, if_undefined), if_defined)| PreprocessorBlock::IfDef {
            ident,
            if_defined,
            if_undefined,
        });

    let expression = {
        let defined = just(SleighToken::Keyword(SleighKeyword::Defined))
            .ignore_then(inline_whitespace_padded(parenthesized(ident)))
            .map(Expression::Defined)
            .map_with(spanned)
            .map(Box::new);

        let comparison_argument = inline_whitespace_padded(
            ident
                .map(ComparisonArgument::Identifier)
                .or(string.map(ComparisonArgument::Literal)),
        );

        let eq_op = just(SleighToken::Operation(SleighOperation::Equals))
            .then(just(SleighToken::Operation(SleighOperation::Equals)))
            .to(true);
        let ne_op = just(SleighToken::Operation(SleighOperation::NotEquals)).to(false);

        let equality_check = comparison_argument
            .clone()
            .map_with(spanned)
            .then(eq_op.or(ne_op))
            .then(comparison_argument.map_with(spanned))
            .map(|((a, equals), b)| match equals {
                true => EqualityCheck::Eq(a, b),
                false => EqualityCheck::Ne(a, b),
            })
            .map(Expression::EqualityCheck)
            .map_with(spanned)
            .map(Box::new);

        let mut expression = Recursive::declare();

        let atom = inline_whitespace_padded(
            defined
                .or(maybe_parenthesized(equality_check))
                .or(parenthesized(expression.clone())),
        );

        let op_and = just(SleighToken::Operation(SleighOperation::And))
            .then(just(SleighToken::Operation(SleighOperation::And)))
            .to(BinaryOp::And);
        let op_or = just(SleighToken::Operation(SleighOperation::Or))
            .then(just(SleighToken::Operation(SleighOperation::Or)))
            .to(BinaryOp::Or);
        let op_xor = just(SleighToken::Operation(SleighOperation::Hat))
            .then(just(SleighToken::Operation(SleighOperation::Hat)))
            .to(BinaryOp::Xor);

        let operator = inline_whitespace_padded(op_and.or(op_or).or(op_xor).map_with(spanned));

        let binary_op = atom
            .clone()
            .foldl_with(operator.then(atom).repeated(), |a, (op, b), extra| {
                Box::new((Expression::BinaryOp(a, op, b), extra.span()))
            });

        expression.define(binary_op);
        expression
    };

    let r#if = pp_macro(SleighKeyword::If, expression.map(unbox))
        .then(blocks.clone().map_with(spanned))
        .then(r#else.clone().map(Some).or(endif.to(None)))
        .map(|((cond, if_true), if_false)| PreprocessorBlock::If {
            cond,
            if_true,
            if_false,
        })
        .boxed();

    blocks.define(
        pcode
            .or(r#if)
            .or(ifdef)
            .or(ifndef)
            .or(define)
            .or(undefine)
            .or(include)
            .or(expansion)
            .map_with(spanned)
            .repeated()
            .collect::<Vec<_>>(),
    );

    blocks
}

fn unbox<T>(boxed: Box<T>) -> T {
    *boxed
}

/// Parse an ident token and return the string id referencing the identifier
fn ident<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, StringId, SleighPreprocessorExtra<'tokens>> + Copy {
    select! { SleighToken::Ident(ident) => ident }.labelled("identifier")
}

/// Parse a sleigh macro. The first parameter is the keyword the macro starts with and
/// the second parameter is the parser that parses the macro contents.
fn pp_macro<'tokens, P, O>(
    macro_keyword: SleighKeyword,
    preprocessor_parser: P,
) -> impl Parser<'tokens, ParserInput<'tokens>, O, SleighPreprocessorExtra<'tokens>> + Clone
where
    P: Parser<'tokens, ParserInput<'tokens>, O, SleighPreprocessorExtra<'tokens>> + Clone,
{
    just(SleighToken::Keyword(macro_keyword))
        .ignore_then(preprocessor_parser)
        .delimited_by(just(SleighToken::MacroBegin), macro_end())
}

/// Parse a sleigh macro that is just a keyword (e.g. `@endif`)
fn pp_macro_kw<'tokens>(
    macro_keyword: SleighKeyword
) -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighPreprocessorExtra<'tokens>> + Clone
where
{
    just(SleighToken::Keyword(macro_keyword))
        .ignored()
        .delimited_by(just(SleighToken::MacroBegin), macro_end())
}

fn macro_end<'tokens>() -> impl Parser<'tokens, ParserInput<'tokens>, (), SleighPreprocessorExtra<'tokens>> + Clone {
    select! {
        SleighToken::Comment(_) => (),
        SleighToken::Newline => (),
    }
    .or(end())
}

fn spanned<'tokens, T>(
    element: T,
    extra: &mut MapExtra<'tokens, '_, ParserInput<'tokens>, SleighPreprocessorExtra<'tokens>>,
) -> (T, InputSpan) {
    (element, extra.span())
}

#[derive(Debug, Clone)]
enum BinaryOp {
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone)]
enum ComparisonArgument {
    Identifier(StringId),
    Literal(StringId),
}

#[derive(Debug, Clone)]
enum EqualityCheck {
    Eq(Spanned<ComparisonArgument>, Spanned<ComparisonArgument>),
    Ne(Spanned<ComparisonArgument>, Spanned<ComparisonArgument>),
}

#[derive(Debug, Clone)]
enum Expression {
    BinaryOp(Box<Spanned<Self>>, Spanned<BinaryOp>, Box<Spanned<Self>>),
    EqualityCheck(EqualityCheck),
    Defined(StringId),
}

#[derive(Debug, Clone)]
enum PreprocessorBlock {
    Define {
        name: Spanned<StringId>,
        value: Spanned<StringId>,
    },
    Undefine(Spanned<StringId>),
    Include(Spanned<StringId>),
    IfDef {
        ident: Spanned<StringId>,
        if_defined: Option<Spanned<Vec<Spanned<Self>>>>,
        if_undefined: Option<Spanned<Vec<Spanned<Self>>>>,
    },
    If {
        cond: Spanned<Expression>,
        if_true: Spanned<Vec<Spanned<Self>>>,
        if_false: Option<Spanned<Vec<Spanned<Self>>>>,
    },
    PCode(Spanned<SleighTokens>),
    Expansion(Spanned<StringId>),
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
