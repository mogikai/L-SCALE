use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::InputSpan;
use crate::spec::builder::StringId;
use crate::spec::moir::common::MOIRSize;
use crate::spec::{
    MOIRSymbol,
    ProcessorSpecificationError,
};
use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
};
use chumsky::span::Span;

pub fn generate_invalid_sizes_error(
    ident: InputSpan,
    expected_size: MOIRSize,
) -> SleighCompilerError {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let report = Report::build(ReportKind::Error, ident.context(), ident.start())
        .with_message(format!("Invalid sizes"))
        .with_label(
            Label::new(ident)
                .with_message(format!("The error occurred here with size {expected_size}"))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_cannot_slice_error(
    ident_str: &str,
    ident: InputSpan,
    expected_size: MOIRSize,
) -> SleighCompilerError {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let report = Report::build(ReportKind::Error, ident.context(), ident.start())
        .with_message(format!("Failed to slice {ident_str} to {expected_size}"))
        .with_label(
            Label::new(ident)
                .with_message(format!("The error occurred here"))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_const_eval_error(
    error: &str,
    lhs: InputSpan,
    rhs: InputSpan,
) -> SleighCompilerError {
    let span = lhs.union(rhs);
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let report = Report::build(ReportKind::Error, span.context(), span.start())
        .with_message(format!("Constant evaluation error: {error}"))
        .with_label(
            Label::new(span)
                .with_message(format!("This went wrong"))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_invalid_symbol_type_error(
    defined_symbol: InputSpan,
    symbol_reference: Spanned<&str>,
    expected: &str,
    found: MOIRSymbol,
) -> SleighCompilerError {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let color_b = colors.next();
    let (sym_ref, sym_ref_span) = symbol_reference.into_tuple();
    let report = Report::build(ReportKind::Error, sym_ref_span.context(), sym_ref_span.start())
        .with_message("Invalid symbol type.")
        .with_label(
            Label::new(defined_symbol)
                .with_message(format!("The symbol is defined here as {}", found.fg(color_a)))
                .with_color(color_a),
        )
        .with_label(
            Label::new(sym_ref_span)
                .with_message(format!(
                    "The symbol referenced by {} has type {}, expected {}.",
                    sym_ref.fg(color_a),
                    found.fg(color_a),
                    expected.fg(color_b)
                ))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_undefined_ident_error(symbol_reference: Spanned<&str>) -> SleighCompilerError {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let (sym_ref, sym_ref_span) = symbol_reference.into_tuple();
    let report = Report::build(ReportKind::Error, sym_ref_span.context(), sym_ref_span.start())
        .with_message("Undefined symbol.")
        .with_label(
            Label::new(sym_ref_span)
                .with_message(format!("There is no definition for {}.", sym_ref.fg(color_a)))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}
