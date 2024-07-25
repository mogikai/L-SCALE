use chumsky::primitive::choice;
use chumsky::Parser;

use crate::compiler::ast::attach::{
    SASTAttach,
    SASTValue,
    SASTVariable,
};
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

use self::name::name;
use self::value::value;
use self::variable::variable;

use super::{
    sleigh_control,
    sleigh_keyword,
    sleigh_whitespace,
    SleighParserExtra,
};

mod name;
mod value;
mod variable;

pub(super) fn attach<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTAttach>, SleighParserExtra<'tokens>> {
    let variable = variable().map(SASTAttach::SASTVariable);
    let value = value().map(SASTAttach::SASTValue);
    let name = name().map(SASTAttach::SASTName);

    let attach = choice((variable, value, name));

    sleigh_keyword(SleighKeyword::Attach)
        .padded_by(sleigh_whitespace())
        .ignore_then(attach)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(|attach, extra| (attach, extra.span()))
        .boxed()
}

#[cfg(test)]
mod test {
    use chumsky::Parser;

    use crate::compiler::parser::report_errors;
    use crate::compiler::parser::test::{
        test_parser,
        TestResult,
    };

    use super::attach;

    fn test_attach(input: &str) -> TestResult {
        test_parser(input, |cache, context, tokens| {
            report_errors(cache, attach().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn attach_variable_parses() -> TestResult {
        test_attach("attach variables [ r64_x reg64_x base64_x index64_x ]   [ R8   R9  R10 R11 R12 R13 R14 R15 ];")?;
        test_attach("attach variables creg  [ CR0 _ CR2 CR3 CR4 _ _ _ ];")
    }

    #[test]
    fn attach_value_parses() -> TestResult {
        test_attach("attach values ss [ 1 2 4 8];")
    }

    #[test]
    fn attach_name_parses() -> TestResult {
        test_attach("attach names xpp [ P4 P5 P6 P7 ];")
    }
}
