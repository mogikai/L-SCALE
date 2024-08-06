use crate::compiler::ast::constructor::bit_pattern_section::SASTBitPatternSection;
use crate::compiler::ast::constructor::disassembly_section::SASTDisassemblySection;
use crate::compiler::ast::item::SASTItem;
use crate::compiler::ast::Spanned;
use crate::spec::StringId;

/// A with block is similar to a constructor. Like the constructor, it specifies a table that it belongs to
/// and also has a bit pattern section and a disassembly section.
/// The table header, bit pattern constraints and disassembly actions are applied to all elements contained
/// in the with block.
#[derive(Debug)]
pub struct SASTWith {
    pub table: Spanned<Option<StringId>>,
    pub bit_pattern_section: Spanned<SASTBitPatternSection>,
    pub disassembly_section: Spanned<SASTDisassemblySection>,
    pub items: Vec<SASTItem>,
}
