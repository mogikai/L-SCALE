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
use self::register::register;
use self::token::token;
use self::user_pcode_op::user_pcode_op;

mod address_space;
mod alignment;
mod bitrange_register;
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

    // combined
    let definitions = choice((endian, alignment, address_space, register, bitrange_register, user_pcode_op, token));

    whitespace_padded(sleigh_keyword(SleighKeyword::Define))
        .ignore_then(definitions)
        .then_ignore(sleigh_control(SleighControlCharacter::SemiColon))
        .map_with(spanned)
}
