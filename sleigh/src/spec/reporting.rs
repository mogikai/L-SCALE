use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
};

use crate::compiler::error::SleighCompilerError;
use crate::spec::moir::endianness::MOIREndianness;
use chumsky::span::Span;


// TODO: Pull the error reporting out of the spec and into the compiler.
//       Also use SleighCompilerError instead of ProcessorSpecificationError
//       The ProcessorSpecificationError should be reserved for the final spec.
/// Generates an error report for IdentAlreadyExists errors.
pub fn generate_identifier_exists_report(
    name: &str,
    old: InputSpan,
    new: InputSpan,
) -> SleighCompilerError {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let report = Report::build(ReportKind::Error, new.context(), new.start())
        .with_message("Identifier already defined")
        .with_label(
            Label::new(new)
                .with_message(format!("Identifier {} is already defined", name.fg(color_a)))
                .with_color(color_a),
        )
        .with_label(
            Label::new(old)
                .with_message(format!("Identifier {} previously defined here", name.fg(color_a)))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_endianness_already_specified_report(
    old: &Spanned<MOIREndianness>,
    new: InputSpan,
) -> SleighCompilerError {
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
            Label::new(old.span().clone())
                .with_message(format!("The endianness is already defined here as {}.", old.fg(color_a)))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}

pub fn generate_alignment_already_specified_report(
    old: &Spanned<u16>,
    new: InputSpan,
) -> SleighCompilerError {
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
            Label::new(old.span().clone())
                .with_message(format!("The alignment is already defined here as {}.", old.fg(color_a)))
                .with_color(color_a),
        )
        .finish();
    SleighCompilerError::ErrorReport(Box::new(report))
}
