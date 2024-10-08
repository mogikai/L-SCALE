use crate::compiler::CompilerContext;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use crate::new_typed_index_table_id;
use crate::spec::builder::StringId;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::space::JIRSpace;
use crate::spec::jir::values::JIRVarNode;
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::bitrange_register::BitrangeRegisterId;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::custom_op::CustomOpId;
use crate::spec::moir::endianness::MOIREndianness;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::r#macro::MacroId;
use crate::spec::moir::table::{
    MOIRTable,
    TableId,
};
use crate::spec::moir::token::TokenId;
use crate::spec::moir::varnode::VarNodeId;
use crate::spec::typed_index_table::TypedIndexTable;
use ariadne::Report;
use builder::MOIRSymbolId;
use std::collections::HashMap;
use string_interner::StringInterner;
use string_interner::backend::StringBackend;
use strum_macros::Display;
use thiserror::Error;

pub mod builder;
pub mod jir;
pub(crate) mod moir;
mod reporting;
pub mod typed_index_table;

new_typed_index_table_id! {
    /// [Ident] is the identifier type that is used by the [ProcessorSpecification]
    /// to refer to individual [JIRSymbolId]s. Each [JIRSymbolId] in turn refers to the corresponding symbol table.
    /// TODO: Improve docs
    pub struct Ident;
    "ID";
}

pub type ProcessorSpecResult<T = ()> = Result<T, ProcessorSpecificationError>;

#[derive(Debug, Clone, Copy, Display, PartialEq)]
pub enum JIRSymbolId {
    Macro(MacroId),
    Space(SpaceId),
    Table(TableId),
    Token(TokenId),
    Field(FieldId),
    VarNode(VarNodeId),
    BitrangeRegister(BitrangeRegisterId),
    CustomOp(CustomOpId),
}

#[derive(Debug, Default)]
pub(crate) struct JIRSymbolTables {
    // macros: TypedIndexTable<MOIRMacroId, MOIRMacro>,
    pub(crate) spaces: TypedIndexTable<SpaceId, JIRSpace>,
    pub(crate) tables: TypedIndexTable<TableId, MOIRTable>,
    pub(crate) constructors: TypedIndexTable<ConstructorId, MOIRConstructor>,
    pub(crate) varnodes: TypedIndexTable<VarNodeId, JIRVarNode>,
    // tokens: TypedIndexTable<MOIRTokenId, MOIRToken>,
    // fields: TypedIndexTable<MOIRFieldId, MOIRField>,
    // custom_ops: TypedIndexTable<MOIRCustomOpId, MOIRCustomOp>,
    // bitrange registers are treated more like placeholders for the corresponding pcode operations to realize access
    // bitrange_registers: TypedIndexTable<MOIRBitrangeRegisterId, MOIRTBitrangeRegister>,
}

#[derive(Debug)]
pub struct ProcessorSpecification {
    pub(crate) interner: StringInterner<StringBackend>,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    pub(crate) symbol_ids: TypedIndexTable<Ident, JIRSymbolId>,

    /// Maps the identifier ids to corresponding strings.
    pub(crate) symbol_strings: HashMap<Ident, StringId>,
    /// Contains a table for each symbol type. A [Ident] maps to a [JIRSymbolId], which in turn maps to the
    /// corresponding [MOIRSymbolTable] with a table-specific id type.
    pub(crate) symbol_tables: JIRSymbolTables,

    /// Maps a string identifier to the corresponding internal [Ident].
    /// This is useful for the compiler, when an identifier string references another symbol.
    /// This will need to be resolved to the correct [Ident], so that the PSB can then
    /// use the internal identifiers, instead of the strings.
    pub(crate) string_to_ident_mapping: HashMap<StringId, Ident>,

    pub(crate) root_table: TableId,

    pub(crate) default_space: SpaceId,
    pub(crate) temp_space: SpaceId,

    pub(crate) endianness: MOIREndianness,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    pub(crate) alignment: u16,
}

impl ProcessorSpecification {
    /// Lookup a varnode by its string identifier.
    /// This lookup is expensive. Consider resolving
    /// to an id first and using the id in subsequent
    /// lookups if performance is critical.
    pub fn varnode_by_string(
        &self,
        name: &str,
    ) -> Option<&JIRVarNode> {
        let name = self.interner.lookup(name)?;
        let ident = self.string_to_ident_mapping.get(&name)?;
        let JIRSymbolId::VarNode(varnode_id) = self.symbol_ids.get(*ident)? else {
            return None;
        };
        self.symbol_tables.varnodes.get(*varnode_id)
    }

    pub fn varnode_by_id(
        &self,
        varnode_id: VarNodeId,
    ) -> ProcessorSpecResult<&JIRVarNode> {
        self.symbol_tables
            .varnodes
            .get(varnode_id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared(JIRSymbolId::VarNode(varnode_id)))
    }

    /// Returns an iterator over all defined spaces.
    pub fn spaces_iter(&self) -> impl Iterator<Item = &JIRSpace> {
        self.symbol_tables.spaces.values()
    }

    /// Returns the largest offset that is used in the provided space by a varnode.
    pub fn space_max_used_offset(
        &self,
        space_id: SpaceId,
    ) -> ProcessorSpecResult<u64> {
        let _ = self
            .symbol_tables
            .spaces
            .get(space_id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared(JIRSymbolId::Space(space_id)))?;
        self.space_varnodes_iter(space_id)?
            .max_by(|a, b| a.end().cmp(&b.end()))
            .map(|a| a.end())
            .ok_or(ProcessorSpecificationError::NoVarNodes)
    }

    /// Return an iterator over all varnodes belonging to the provided space id.
    /// Returns an error if the space does not exist.
    pub fn space_varnodes_iter(
        &self,
        space_id: SpaceId,
    ) -> ProcessorSpecResult<impl Iterator<Item = &JIRVarNode>> {
        Ok(self
            .symbol_tables
            .varnodes
            .values()
            .filter(move |varnode| varnode.space_id == space_id))
    }

    pub fn list_global_varnode_strings(&self) -> Vec<&str> {
        // TODO: Maybe we can include some backwards mappings to make these lookups easier?
        //       although they most likely are only used in rare cases.
        self.symbol_ids
            .iter()
            .filter_map(|(id, sym)| {
                if let JIRSymbolId::VarNode(_) = sym {
                    let string_id = *self
                        .symbol_strings
                        .get(&id)
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
    #[error("The symbol id does not map to a symbol.")]
    SymbolUndeclared(JIRSymbolId),
    #[error("The identifier is not mapped to anything.")]
    IdentifierNotMapped(Spanned<StringId>),
    #[error("The symbol exists but its type does not match.")]
    InvalidSymbolType(MOIRSymbolId),
    #[error("The processor specification builder produced a detailed error message:")]
    ErrorReport(Box<Report<'static, InputSpan>>),
    #[error("The processor specification must contain a valid endianness definition.")]
    MissingEndiannessDefinition,
    #[error("The processor specification must contain exactly one default space, but none was defined.")]
    MissingDefaultSpace,
    #[error("The specification does not specify any varnodes.")]
    NoVarNodes,
    #[error("The instruction is missing an output varnode")]
    MissingOutput,
    #[error("The instruction is missing an input varnode or constant")]
    MissingInput,
    #[error("The block with id {0} is not defined yet")]
    UndefinedBlock(JIRBlockId),
}
