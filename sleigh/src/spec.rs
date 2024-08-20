use ariadne::Report;
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::collections::HashMap;
use string_interner::backend::StringBackend;
use string_interner::StringInterner;
use strum_macros::Display;
use thiserror::Error;

use self::moir::bitrange_register::MOIRBitrangeRegisterId;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use crate::compiler::CompilerContext;
use crate::spec::builder::{
    StringId,
    SymbolTables,
};
use crate::spec::moir::address_space::MOIRSpaceId;
use crate::spec::moir::custom_op::MOIRCustomOpId;
use crate::spec::moir::endianness::MOIREndianness;
use crate::spec::moir::r#macro::MOIRMacroId;
use crate::spec::moir::table::MOIRTableId;
use crate::spec::moir::varnode::{
    MOIRVarNode,
    MOIRVarNodeId,
};
use moir::field::MOIRFieldId;
use moir::token::MOIRTokenId;

pub mod builder;
pub mod jir;
pub(crate) mod moir;
mod reporting;
pub mod typed_index_table;

new_key_type! {
    /// [MOIRIdent] is the identifier type that is used by the [ProcessorSpecification] and [ProcessorSpecificationBuilder]
    /// to refer to individual [MOIRSymbol]s. Each [MOIRSymbol] in turn refers to the corresponding symbol table.
    /// The [ProcessorSpecification] can be used to fetch a symbol for a given [MOIRIdent].
    /// TODO: Improve docs
    pub struct MOIRIdent;
}

#[derive(Debug, Clone, Copy, Display, PartialEq)]
pub enum MOIRSymbol {
    /// The identifier is not defined yet and can be used to define a symbol.
    Undefined,
    Macro(MOIRMacroId),
    Space(MOIRSpaceId),
    Table(MOIRTableId),
    Token(MOIRTokenId),
    Field(MOIRFieldId),
    VarNode(MOIRVarNodeId),
    BitrangeRegister(MOIRBitrangeRegisterId),
    CustomOp(MOIRCustomOpId),
}

#[derive(Debug)]
pub struct ProcessorSpecification {
    interner: StringInterner<StringBackend>,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    symbols: SlotMap<MOIRIdent, MOIRSymbol>,

    /// Maps the identifier ids to corresponding strings.
    symbol_strings: SecondaryMap<MOIRIdent, Spanned<StringId>>,
    /// Contains a table for each symbol type. A [MOIRIdent] maps to a [MOIRSymbol], which in turn maps to the
    /// corresponding [MOIRSymbolTable] with a table-specific id type.
    symbol_tables: SymbolTables,

    /// Maps a string identifier to the corresponding internal [MOIRIdent].
    /// This is useful for the compiler, when an identifier string references another symbol.
    /// This will need to be resolved to the correct [MOIRIdent], so that the PSB can then
    /// use the internal identifiers, instead of the strings.
    string_to_ident_mapping: HashMap<StringId, MOIRIdent>,

    root_table: StringId,

    default_space: MOIRIdent,

    endianness: MOIREndianness,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    alignment: u16,
}

impl ProcessorSpecification {
    /// Lookup a varnode by its string identifier.
    /// This lookup is expensive. Consider resolving
    /// to an id first and using the id in subsequent
    /// lookups if performance is critical.
    pub fn varnode_by_string(
        &self,
        name: &str,
    ) -> Option<&MOIRVarNode> {
        let name = self.interner.lookup(name)?;
        let ident = self.string_to_ident_mapping.get(&name)?;
        let MOIRSymbol::VarNode(varnode_id) = self.symbols.get(*ident)? else {
            return None;
        };
        self.symbol_tables.varnodes.get(*varnode_id)
    }

    pub fn list_global_varnode_strings(&self) -> Vec<&str> {
        // TODO: Maybe we can include some backwards mappings to make these lookups easier?
        //       although they most likely are only used in rare cases.
        self.symbols
            .iter()
            .filter_map(|(id, sym)| {
                if let MOIRSymbol::VarNode(_) = sym {
                    let string_id = **self
                        .symbol_strings
                        .get(id)
                        .expect("The ident must have a corresponding string mapped.");
                    Some(
                        self.interner
                            .string(string_id)
                            .expect("The string must have been interned."),
                    )
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The requested identifier is already used by something else.")]
    IdentAlreadyExists(InputSpan, InputSpan),
    #[error("The symbol must first be declared before it can be used.")]
    SymbolUndeclared,
    #[error("The identifier is not mapped to anything.")]
    IdentifierNotMapped(Spanned<StringId>),
    #[error("The symbol exists but its type does not match.")]
    InvalidSymbolType(MOIRSymbol),
    #[error("The processor specification builder produced a detailed error message:")]
    ErrorReport(Box<Report<'static, InputSpan>>),
    #[error("The processor specification must contain a valid endianness definition.")]
    MissingEndiannessDefinition,
    #[error("The processor specification must contain exactly one default space, but none was defined.")]
    MissingDefaultSpace,
}
