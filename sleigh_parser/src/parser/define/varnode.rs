use crate::Spanned;
use crate::ast::define::SASTVarnode;
use crate::lexer::ParserInput;
use crate::parser::helpers::spanned;
use crate::parser::{
    SleighParserExtra,
    sast_ident_spanned,
    sleigh_control,
    sleigh_keyword,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
};
use crate::tokens::{
    SleighControlCharacter,
    SleighKeyword,
    SleighOperation,
};
use chumsky::{
    IterParser,
    Parser,
};

pub fn varnodes<'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTVarnode>, SleighParserExtra<'tokens>> {
    let stringlist = sleigh_control(SleighControlCharacter::OpenBracket)
        .ignore_then(
            whitespace_padded(sast_ident_spanned())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(sleigh_control(SleighControlCharacter::CloseBracket));

    let names = whitespace_padded(sast_ident_spanned()).map(|name| vec![name]);

    let offset = sleigh_keyword(SleighKeyword::Offset)
        .ignore_then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .ignore_then(sleigh_number())
        .map_with(spanned);

    let size = sleigh_keyword(SleighKeyword::Size)
        .ignore_then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .ignore_then(sleigh_number())
        .map_with(spanned);

    whitespace_padded(sast_ident_spanned())
        .then(whitespace_padded(offset))
        .then(whitespace_padded(size))
        .then(names.or(stringlist))
        .map(|(((spacename, offset), size), names)| SASTVarnode {
            spacename,
            offset,
            size,
            names,
        })
        .map_with(spanned)
        .boxed()
}

#[cfg(test)]
mod test {
    use chumsky::Parser;

    use super::*;
    use crate::parser::test::TestResult;

    fn test_varnode(expression: &str) -> TestResult {
        use crate::parser::test::{
            report_errors,
            test_parser,
        };
        test_parser(expression, |cache, context, tokens| {
            report_errors(cache, varnodes().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn varnode_parses() -> TestResult {
        test_varnode("register offset=0x2000 size=8 contextreg")?;
        test_varnode("ram offset=0x2000 size=1 contextreg")?;
        Ok(())
    }
}
