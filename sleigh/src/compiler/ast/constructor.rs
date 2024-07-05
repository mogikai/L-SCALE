use crate::compiler::ast::constructor::bit_pattern_section::SASTBitPatternSection;
use crate::compiler::ast::constructor::disassembly_section::SASTDisassemblySection;
use crate::compiler::ast::constructor::display_section::SASTDisplaySection;
use crate::compiler::ast::constructor::semantic_section::SASTSemanticSection;
use crate::compiler::ast::Spanned;

pub mod bit_pattern_section;
pub mod disassembly_section;
pub mod display_section;
pub mod semantic_section;

#[derive(Debug)]
pub struct SASTConstructor<'src> {
    /// Every constructor must be part of a table. This field specifies which
    /// table this constructor belongs to. A constructor in the root instruction
    /// table is specified as an empty string in the specification, which
    /// receives the `None` value in the ast.
    pub table: Spanned<'src, Option<&'src str>>,
    pub display_section: Spanned<'src, SASTDisplaySection<'src>>,
    pub bit_pattern_section: Spanned<'src, SASTBitPatternSection<'src>>,
    pub disassembly_section: Spanned<'src, SASTDisassemblySection<'src>>,
    pub semantic_section: Spanned<'src, SASTSemanticSection<'src>>,
}
