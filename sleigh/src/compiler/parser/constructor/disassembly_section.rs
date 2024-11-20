use chumsky::recursive::recursive;
use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::disassembly_section::{
    SASTAction,
    SASTDisassemblySection,
    SASTPatternExpression,
};
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::parser::{
    SleighParserExtra,
    sast_ident,
    sast_ident_spanned,
    sleigh_control,
    sleigh_number,
    sleigh_operation,
    whitespace_padded,
};
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighOperation,
};

pub fn disassembly_section<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDisassemblySection>, SleighParserExtra<'tokens>> {
    // TODO: This needs some cleanup.
    let expression = recursive(|expr| {
        let terminal = sast_ident()
            .map(SASTPatternExpression::Symbol)
            .or(sleigh_number()
                .map(|num: u64| num as _)
                .map(SASTPatternExpression::Integer))
            .map_with(spanned)
            .boxed();

        let atom = terminal.or(expr.delimited_by(
            whitespace_padded(sleigh_control(SleighControlCharacter::OpenParen)),
            whitespace_padded(sleigh_control(SleighControlCharacter::CloseParen)),
        ));

        // ordered by precedence (unary first)
        let unary = sleigh_operation(SleighOperation::BitNegate)
            .or(sleigh_operation(SleighOperation::Not))
            .repeated()
            .foldr_with(atom, |_, rhs, extra| Spanned::new(SASTPatternExpression::Not(Box::new(rhs)), extra.span()))
            .boxed();

        // TODO: Each of these parsers can most likely be unified into one binary operation parser.

        let product = unary
            .clone()
            .foldl_with(
                whitespace_padded(sleigh_operation(SleighOperation::Star))
                    .to(SASTPatternExpression::Mul as fn(_, _) -> _)
                    .or(whitespace_padded(sleigh_operation(SleighOperation::Division))
                        .to(SASTPatternExpression::Div as fn(_, _) -> _))
                    .then(unary)
                    .repeated(),
                |lhs, (op, rhs), extra| Spanned::new(op(Box::new(lhs), Box::new(rhs)), extra.span()),
            )
            .boxed();

        let sum = product
            .clone()
            .foldl_with(
                whitespace_padded(sleigh_operation(SleighOperation::Plus))
                    .to(SASTPatternExpression::Add as fn(_, _) -> _)
                    .or(whitespace_padded(sleigh_operation(SleighOperation::Minus))
                        .to(SASTPatternExpression::Sub as fn(_, _) -> _))
                    .then(product)
                    .repeated(),
                |lhs, (op, rhs), extra| Spanned::new(op(Box::new(lhs), Box::new(rhs)), extra.span()),
            )
            .boxed();

        let shl = whitespace_padded(
            sleigh_operation(SleighOperation::LessThan).then(sleigh_operation(SleighOperation::LessThan)),
        )
        .ignored();
        let shr = whitespace_padded(
            sleigh_operation(SleighOperation::GreaterThan).then(sleigh_operation(SleighOperation::GreaterThan)),
        )
        .ignored();

        let shift = sum
            .clone()
            .foldl_with(
                shl.to(SASTPatternExpression::Shl as fn(_, _) -> _)
                    .or(shr.to(SASTPatternExpression::Shr as fn(_, _) -> _))
                    .then(sum)
                    .repeated(),
                |lhs, (op, rhs), extra| Spanned::new(op(Box::new(lhs), Box::new(rhs)), extra.span()),
            )
            .boxed();

        let and = shift
            .clone()
            .foldl_with(
                whitespace_padded(sleigh_operation(SleighOperation::BitAnd).or(sleigh_operation(SleighOperation::And)))
                    .then(shift)
                    .repeated(),
                |lhs, (_, rhs), extra| {
                    Spanned::new(SASTPatternExpression::And(Box::new(lhs), Box::new(rhs)), extra.span())
                },
            )
            .boxed();

        let xor = and.clone().foldl_with(
            whitespace_padded(sleigh_operation(SleighOperation::BitXor).or(sleigh_operation(SleighOperation::Hat)))
                .then(and)
                .repeated(),
            |lhs, (_, rhs), extra| Spanned::new(SASTPatternExpression::Xor(Box::new(lhs), Box::new(rhs)), extra.span()),
        );

        let or = xor
            .clone()
            .foldl_with(
                whitespace_padded(sleigh_operation(SleighOperation::BitOr).or(sleigh_operation(SleighOperation::Or)))
                    .then(xor)
                    .repeated(),
                |lhs, (_, rhs), extra| {
                    Spanned::new(SASTPatternExpression::Or(Box::new(lhs), Box::new(rhs)), extra.span())
                },
            )
            .boxed();

        or
    });

    let action = sast_ident_spanned()
        .then(whitespace_padded(sleigh_operation(SleighOperation::Equals)))
        .then(expression)
        .then_ignore(whitespace_padded(sleigh_control(SleighControlCharacter::SemiColon)))
        .map(|((ident, _), e)| SASTAction { lhs: ident, rhs: e })
        .map_with(spanned)
        .boxed();

    whitespace_padded(
        whitespace_padded(action)
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(
                sleigh_control(SleighControlCharacter::OpenBracket),
                sleigh_control(SleighControlCharacter::CloseBracket),
            )
            .or_not()
            .map(|actions| SASTDisassemblySection { actions })
            .map_with(spanned),
    )
    .boxed()
}

#[cfg(test)]
mod test {
    use chumsky::Parser;

    use crate::compiler::parser::report_errors;
    use crate::compiler::parser::test::{
        TestResult,
        test_parser,
    };

    use super::disassembly_section;

    fn test_disassembly(input: &str) -> TestResult {
        test_parser(input, |cache, context, tokens| {
            report_errors(cache, disassembly_section().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn disassembly_simple_operations_parse() -> TestResult {
        test_disassembly("[ reloc = 3 + 8; ]")?;
        test_disassembly("[ reloc = 3 - 8; ]")?;
        test_disassembly("[ reloc = 3 * 8; ]")?;
        test_disassembly("[ reloc = 3 / 8; ]")?;
        test_disassembly("[ reloc = 3 << 8; ]")?;
        test_disassembly("[ reloc = 3 >> 8; ]")?;
        test_disassembly("[ reloc = 3 & 8; ]")?;
        test_disassembly("[ reloc = 3 $and 8; ]")?;
        test_disassembly("[ reloc = 3 | 8; ]")?;
        test_disassembly("[ reloc = 3 $or 8; ]")?;
        test_disassembly("[ reloc = 3 ^ 8; ]")?;
        test_disassembly("[ reloc = 3 $xor 8; ]")?;
        test_disassembly("[ reloc = ~3; ]")?;

        test_disassembly("[ reloc = !!~~~!3 ; ]")?;
        Ok(())
    }

    #[test]
    fn disassembly_whitespace_parses() -> TestResult {
        test_disassembly("[reloc=3+8;]")?;
        test_disassembly("[ reloc=3+8;]")?;
        test_disassembly("[reloc=3+8; ]")?;
        test_disassembly("[reloc =3+8;]")?;
        test_disassembly("[reloc= 3+8;]")?;
        test_disassembly("[reloc=3 +8;]")?;
        test_disassembly("[reloc=3+ 8;]")?;
        test_disassembly("[reloc=3+8 ;]")?;
        test_disassembly("[reloc=3+8; ]")?;
        Ok(())
    }

    #[test]
    fn disassembly_combined_expression_parses() -> TestResult {
        test_disassembly(
            "[reloc = (((9 & 0x1234) + base) * (!!(1 $or 8) / ((3 $xor displace / ~19))) << (3 | 1) ^ 0xffff) >> (4);]",
        )?;
        Ok(())
    }
}
