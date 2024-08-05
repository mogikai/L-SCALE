use chumsky::{extra, IterParser, Parser, select};
use chumsky::prelude::{any, end, just, Recursive};
use chumsky::error::Rich;
use chumsky::input::MapExtra;
use crate::compiler::{CompilerContext, StringId};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::{InputSpan, ParserInput, SleighTokens};
use crate::compiler::parser::{inline_whitespace_padded, maybe_parenthesized, parenthesized};
use crate::compiler::tokens::{SleighKeyword, SleighOperation, SleighToken};
use crate::preprocessor::blocks::{BinaryOp, ComparisonArgument, EqualityCheck, Expression, PreprocessorBlock};
use crate::spec::ProcessorSpecificationBuilder;

type SleighPreprocessorExtra<'tokens> = extra::Full<Rich<'tokens, SleighToken, InputSpan>, ProcessorSpecificationBuilder, ()>;

pub fn parser<'tokens>(
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
        .map_with(|tokens, extra| PreprocessorBlock::PCode(Spanned::new(SleighTokens::from(tokens), extra.span())));

    let expansion =
        select! {SleighToken::MacroExpansion(ident) = extra => PreprocessorBlock::Expansion(Spanned::new(ident, extra.span())) };

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
                Box::new(Spanned::new(Expression::BinaryOp(a, op, b), extra.span()))
            });

        expression.define(binary_op);
        expression.boxed()
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
            .boxed()
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
) -> Spanned<T> {
    Spanned::new(element, extra.span())
}