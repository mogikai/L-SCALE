use ariadne::Report;
use slotmap::new_key_type;
use thiserror::Error;

use moir::field::MOIRFieldId;
use moir::r#macro::MOIRMacro;
use moir::table::MOIRTable;
use moir::token::MOIRTokenId;
use moir::varnode::MOIRVarNode;

use crate::compiler::lexer::InputSpan;
use crate::spec::moir::address_space::MOIRAddressSpaceId;
use crate::spec::moir::r#macro::MOIRMacroId;
use crate::spec::moir::table::MOIRTableId;
use crate::spec::symbol_table::MOIRSymbolTable;

pub mod builder;
pub mod moir;
mod reporting;
mod symbol_table;

new_key_type! {
    /// [MOIRIdent] is the identifier type that is used by the [ProcessorSpecification] and [ProcessorSpecificationBuilder]
    /// to refer to individual [MOIRSymbol]s. Each [MOIRSymbol] in turn refers to the corresponding symbol table.
    /// The [ProcessorSpecification] can be used to fetch a symbol for a given [MOIRIdent].
    /// TODO: Improve docs
    pub struct MOIRIdent;
}

#[derive(Debug)]
pub enum MOIRSymbol {
    /// The identifier is not defined yet and can be used to define a symbol.
    Undefined,
    Macro(MOIRMacroId),
    Space(MOIRAddressSpaceId),
    Table(MOIRTableId),
    Token(MOIRTokenId),
    Field(MOIRFieldId),
}

#[derive(Debug, Default)]
pub struct ProcessorSpecification {}

#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The requested identifier is already used by something else.")]
    IdentAlreadyExists(InputSpan, InputSpan),
    #[error("The symbol must first be declared before it can be defined")]
    SymbolUndeclared,
    #[error("The processor specification builder produced a detailed error message:")]
    ErrorReport(Box<Report<'static, InputSpan>>),
}
