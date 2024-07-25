use chumsky::primitive::choice;
use chumsky::{
    IterParser,
    Parser,
};

use crate::compiler::ast::define::SASTDefine;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::ParserInput;
use crate::compiler::parser::define::endianness::endianness;
use crate::compiler::parser::helpers::spanned;
use crate::compiler::tokens::{
    SleighControlCharacter,
    SleighKeyword,
};

use super::{
    sleigh_control,
    sleigh_keyword,
    whitespace_padded,
    SleighParserExtra,
};

use self::address_space::address_space;
use self::alignment::alignment;
use self::bitrange_register::bitrange_register;
use self::context::context;
use self::register::register;
use self::token::token;
use self::user_pcode_op::user_pcode_op;

mod address_space;
mod alignment;
mod bitrange_register;
mod context;
mod endianness;
mod register;
mod token;
mod user_pcode_op;

pub(super) fn define<'tokens>(
) -> impl Parser<'tokens, ParserInput<'tokens>, Spanned<SASTDefine>, SleighParserExtra<'tokens>> {
    // basic definitions
    let endian = endianness().map(SASTDefine::Endianness);
    let alignment = alignment().map(SASTDefine::Alignment);
    let address_space = address_space().map(SASTDefine::AddressSpace);
    let register = register().map(SASTDefine::Register);
    let bitrange_register = bitrange_register().map(SASTDefine::BitrangeRegister);
    let user_pcode_op = user_pcode_op().map(SASTDefine::UserPcodeOp);

    // token
    let token = token().map(SASTDefine::Token);

    // context variables
    let context = context().map(SASTDefine::Context);

    // combined
    let definitions =
        choice((endian, alignment, address_space, register, bitrange_register, user_pcode_op, token, context));

    whitespace_padded(sleigh_keyword(SleighKeyword::Define))
        .ignore_then(definitions)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(spanned)
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

    use super::define;

    fn test_define(input: &str) -> TestResult {
        test_parser(input, |cache, context, tokens| {
            report_errors(cache, define().parse_with_state(tokens.spanned(), context))
        })?;
        Ok(())
    }

    #[test]
    fn define_endian_parses() -> TestResult {
        test_define("define endian=little;")?;
        test_define("define endian=big;")
    }

    #[test]
    fn define_alignment_parses() -> TestResult {
        test_define("define alignment=2;")?;
        test_define("define alignment=32;")?;
        test_define("define alignment=128;")
    }

    #[test]
    fn define_address_space_parses() -> TestResult {
        test_define("define space ram type=ram_space size=4 default;")?;
        test_define("define space register type=register_space size=4;")
    }

    #[test]
    fn define_register_parses() -> TestResult {
        test_define("define register offset=0 size=4 [EAX ECX EDX EBX ESP EBP ESI EDI ];")?;
        test_define("define register offset=0 size=2 [AX _ CX _ DX _ BX _ SP _ BP _ SI _ DI];")?;
        test_define("define register offset=0 size=1 [AL AH _ _ CL CH _ _ DL DH _ _ BL BH ];")
    }

    #[test]
    fn define_bitrange_register_parses() -> TestResult {
        test_define("define register offset=0x180 size=4 [ statusreg ];")?;
        test_define("define bitrange zf=statusreg[10,1] cf=statusreg[11,1] sf=statusreg[12,1];")
    }

    #[test]
    fn define_user_pcode_op_parses() -> TestResult {
        test_define("define pcodeop arctan;")
    }

    #[test]
    fn define_token_parses() -> TestResult {
        test_define("define token opbyte (8) endian=little byte=(0,7) high4=(4,7) signed hex high5=(3,7) hex low5=(0,4) dec byte_4=(4,4) byte_0=(0,0);")?;
        test_define("define token I32 (32) imm32=(0,31) simm32=(0,31) signed hex;")
    }

    #[test]
    fn define_context_parses() -> TestResult {
        // TODO: the actual specification (x86/ia.sinc) uses "contextreg" as a name
        // although it contains a reserved keyword. What do to?
        test_define("define context contexxreg longMode=(0,0) hex reserved=(1,3) dec signed addrsize=(4,5) repprefx=(13,13) noflow prefix_66=(14,14) mandover=(12,14);")
    }
}
