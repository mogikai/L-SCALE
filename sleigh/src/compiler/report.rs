use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::InputSpan;
use crate::spec::ProcessorSpecificationError;
use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
};
use chumsky::span::Span;

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
