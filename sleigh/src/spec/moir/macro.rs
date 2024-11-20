use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::pcode::SASTPCode;
use crate::new_typed_index_table_id;

#[derive(Debug, Clone)]
pub struct MOIRMacro {
    /// The arguments are simply a list of named identifiers that are mapped to concrete locals on invocation.
    pub arguments: Vec<Spanned<SASTIdent>>,
    /// Macros contain the AST for the respective pcode, so it can be atomized on the fly in the correct context.
    pub pcode: SASTPCode,
}

new_typed_index_table_id! { pub struct MacroId; "M"; }
