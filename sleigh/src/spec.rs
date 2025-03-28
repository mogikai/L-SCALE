use self::jir::jumplist::{
    Jumplist,
    JumplistId,
};
use self::moir::field::MOIRField;
use crate::compiler::InternalCompilerResult;
use crate::lifter::LifterDiagnostics;
use crate::lifter::context_provider::ContextProvider;
use crate::new_typed_index_table_id;
use crate::spec::builder::MOIRSymbolId;
use crate::spec::jir::context_stack::ContextStack;
use crate::spec::jir::custom_op::JIRCustomOp;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::space::JIRSpace;
use crate::spec::jir::values::{
    JIRSlice,
    JIRVarNode,
};
pub use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::bitrange_register::BitrangeRegisterId;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::ContextId;
pub use crate::spec::moir::custom_op::CustomOpId;
use crate::spec::moir::endianness::MOIREndianness;
pub use crate::spec::moir::field::FieldId;
use crate::spec::moir::r#macro::MacroId;
use crate::spec::moir::table::{
    MOIRTable,
    TableId,
};
use crate::spec::moir::token::TokenId;
use crate::spec::moir::varnode::VarNodeId;
use brotli::Decompressor;
use itertools::Itertools;
use jir::jumplist::MatchContext;
use jir::values::{
    VarnodeGroup,
    VarnodeGroupId,
};
use mogi_sleigh_parser::{
    ParserContext,
    SleighInterner,
    Spanned,
    StringId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use mogi_utilities::new_map_key;
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::{
    Read,
    Write,
};
use std::ops::Range;
use strum_macros::Display;
use thiserror::Error;
use tracing::debug;

pub mod builder;
mod disassembler;
pub mod jir;
mod lifter;
pub mod moir;
pub mod typed_index_table;

new_map_key! {
    /// [Ident] is the identifier type that is used by the [ProcessorSpecification]
    /// to refer to individual [JIRSymbolId]s. Each [JIRSymbolId] in turn refers to the corresponding symbol table.
    /// TODO: Improve docs
    pub struct Ident;
    "ID";
}

pub type ProcessorSpecResult<T = ()> = Result<T, ProcessorSpecificationError>;

#[derive(Debug, Clone, Copy, Display, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct JIRSymbolTables {
    // macros: TypedIndexTable<MOIRMacroId, MOIRMacro>,
    pub(crate) spaces: PrimaryMap<SpaceId, JIRSpace>,
    // pub(crate) tables: TypedIndexTable<TableId, MOIRTable>,
    pub(crate) constructors: PrimaryMap<ConstructorId, MOIRConstructor>,
    pub(crate) varnodes: PrimaryMap<VarNodeId, JIRVarNode>,
    pub(crate) fields: PrimaryMap<FieldId, MOIRField>,
    // tokens: TypedIndexTable<MOIRTokenId, MOIRToken>,
    // fields: TypedIndexTable<MOIRFieldId, MOIRField>,
    custom_ops: PrimaryMap<CustomOpId, Option<JIRCustomOp>>,
    // bitrange registers are treated more like placeholders for the corresponding pcode operations to realize access
    // bitrange_registers: TypedIndexTable<MOIRBitrangeRegisterId, MOIRTBitrangeRegister>,
}

impl JIRSymbolTables {
    pub fn shrink_to_fit(&mut self) {
        self.spaces.shrink_to_fit();
        self.constructors.shrink_to_fit();
        self.varnodes.shrink_to_fit();
        self.fields.shrink_to_fit();
        self.custom_ops.shrink_to_fit();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorSpecification {
    pub(crate) interner: SleighInterner,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    pub(crate) symbol_ids: PrimaryMap<Ident, JIRSymbolId>,

    /// Maps the identifier ids to corresponding strings.
    pub(crate) symbol_strings: HashMap<Ident, StringId>,
    /// Contains a table for each symbol type. An [Ident] maps to a [JIRSymbolId], which in turn maps to the
    /// corresponding [MOIRSymbolTable] with a table-specific id type.
    pub(crate) symbol_tables: JIRSymbolTables,
    /// A mapping to look up context ids from their varnode id.
    pub(crate) contexts: HashMap<VarNodeId, ContextId>,

    /// Overlapping varnodes need special treatments in multiple locations, which
    /// is why we keep track of different disjoint groups.
    pub(crate) varnode_groups: PrimaryMap<VarnodeGroupId, VarnodeGroup>,
    pub(crate) varnode_to_group: SecondaryMap<VarNodeId, Option<VarnodeGroupId>>,

    /// Maps varnode ids to the corresponding string id, such that we can display them to the user.
    pub(crate) varnode_names: SecondaryMap<VarNodeId, Option<StringId>>,

    /// Maps a string identifier to the corresponding internal [Ident].
    /// This is useful for the compiler, when an identifier string references another symbol.
    /// This will need to be resolved to the correct [Ident], so that the PSB can then
    /// use the internal identifiers, instead of the strings.
    pub(crate) string_to_ident_mapping: HashMap<StringId, Ident>,

    /// The first jumplist is the root from which matching begins.
    pub(crate) jumplist_cache: PrimaryMap<JumplistId, Jumplist>,

    pub(crate) root_table: TableId,

    pub(crate) default_space: SpaceId,
    pub(crate) temp_space: SpaceId,

    pub(crate) endianness: MOIREndianness,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    pub(crate) alignment: u16,
    #[serde(skip_serializing, skip_deserializing)]
    lifter_diagnostics: RefCell<LifterDiagnostics>,
}

impl ProcessorSpecification {
    pub fn context_id(
        &self,
        context_name: &str,
    ) -> Option<ContextId> {
        self.contexts
            .get(&self.varnode_id_by_string(context_name)?)
            .copied()
    }

    /// Gets the slice associated with the program counter of the cpu.
    pub fn program_counter_slice(&self) -> JIRSlice {
        // TODO: Read this from somewhere instead of hardcoding it. Currently x64 specific
        self.varnode_slice_by_string("RIP").unwrap()
    }

    /// Create a new [MatchContext] from the given [EVAddr].
    pub(crate) fn new_match_context(
        &self,
        addr: EVAddr,
    ) -> ProcessorSpecResult<MatchContext> {
        Ok(MatchContext::new(addr, &self.jumplist_cache, &self.symbol_tables.constructors))
    }

    pub fn default_space(&self) -> SpaceId {
        self.default_space
    }

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

    /// Lookup a varnode by its string identifier and return a [JIRSlice] to it.
    /// This lookup is expensive.
    pub fn varnode_slice_by_string(
        &self,
        name: &str,
    ) -> Option<JIRSlice> {
        let name = self.interner.lookup(name)?;
        let ident = self.string_to_ident_mapping.get(&name)?;
        let JIRSymbolId::VarNode(varnode_id) = self.symbol_ids.get(*ident).copied()? else {
            return None;
        };
        let varnode = self.symbol_tables.varnodes.get(varnode_id)?;
        Some(JIRSlice::new(
            varnode.space_id,
            self.varnode_to_group[varnode_id].expect("Every varnode must be in a group."),
            varnode
                .offset
                .try_into()
                .expect("The address space needs to have been compressed at this point"),
            varnode.size,
        ))
    }

    /// Lookup a varnode by its id and return a slice to it.
    pub fn varnode_slice_by_id(
        &self,
        id: VarNodeId,
    ) -> ProcessorSpecResult<JIRSlice> {
        self.symbol_tables
            .varnodes
            .get(id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared(JIRSymbolId::VarNode(id)))
            .map(|varnode| {
                JIRSlice::new(
                    varnode.space_id,
                    self.varnode_to_group[id].expect("Every varnode must be in a group."),
                    varnode
                        .offset
                        .try_into()
                        .expect("The address space needs to have been compressed at this point"),
                    varnode.size,
                )
            })
    }

    /// Lookup a varnode id by its string identifier.
    /// This lookup is expensive. Consider using the
    /// id in subsequent lookups if performance is critical.
    pub fn varnode_id_by_string(
        &self,
        name: &str,
    ) -> Option<VarNodeId> {
        let name = self.interner.lookup(name)?;
        let ident = self.string_to_ident_mapping.get(&name)?;
        let JIRSymbolId::VarNode(varnode_id) = self.symbol_ids.get(*ident)? else {
            return None;
        };
        Some(*varnode_id)
    }

    /// Look up a field by its string identifier.
    pub fn field_id_by_string(
        &self,
        name: &str,
    ) -> Option<FieldId> {
        let name = self.interner.lookup(name)?;
        let ident = self.string_to_ident_mapping.get(&name)?;
        match self.symbol_ids.get(*ident)? {
            JIRSymbolId::Field(field_id) => Some(*field_id),
            _ => None,
        }
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

    pub fn debug_space_allocation(
        &self,
        space_id: SpaceId,
    ) -> ProcessorSpecResult<()> {
        use std::fmt::Write;
        let mut group_dbg = String::new();
        let mut prev_end = 0;
        for group in self.varnode_groups.values() {
            if group.offset != prev_end {
                writeln!(group_dbg, "Empty space of {} bytes", group.offset - prev_end).unwrap();
            }
            write!(group_dbg, "Group<{}..={}>[", group.offset, group.end().saturating_sub(1)).unwrap();
            for varnode_id in &group.varnodes {
                let str = self
                    .interner
                    .string_unchecked(self.varnode_names[*varnode_id].unwrap());
                write!(group_dbg, "{} ", str).unwrap();
            }
            writeln!(group_dbg, "]").unwrap();
            prev_end = group.end();
        }

        debug!("Space {} consists of the following groups: \n{}", space_id, group_dbg);

        let _ = self
            .symbol_tables
            .spaces
            .get(space_id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared(JIRSymbolId::Space(space_id)))?;
        let merged = merge_ranges(
            self.space_varnodes_iter(space_id)?
                .map(|varnode| varnode.offset..varnode.end())
                .collect::<Vec<_>>(),
        );

        debug!("Varnodes occupy the following ranges: {:#05x?}", merged);
        let total_size: u64 = merged.iter().map(|r| r.end - r.start).sum();
        debug!("Total size: {}", total_size);
        debug!("Potential savings: {}", self.space_max_used_offset(space_id)? - total_size);
        Ok(())
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
            .items()
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

    pub fn jumplist_cache(&self) -> &PrimaryMap<JumplistId, Jumplist> {
        &self.jumplist_cache
    }

    pub fn constructors(&self) -> &PrimaryMap<ConstructorId, MOIRConstructor> {
        &self.symbol_tables.constructors
    }

    pub fn shrink(&mut self) {
        self.interner.shrink_to_fit();
        self.symbol_ids.shrink_to_fit();
        self.symbol_strings.shrink_to_fit();
        self.symbol_tables.shrink_to_fit();
        self.contexts.shrink_to_fit();
        self.string_to_ident_mapping.shrink_to_fit();
        self.jumplist_cache.shrink_to_fit();
    }

    /// Saves the processor specification to the target writer.
    pub fn save<W: Write>(
        &self,
        writer: &mut W,
    ) {
        let mut writer = brotli::CompressorWriter::new(writer, 4096 * 1000, 5, 22);
        bincode::serde::encode_into_std_write(self, &mut writer, bincode::config::standard()).unwrap();
    }

    pub fn load<R: Read>(reader: &mut R) -> ProcessorSpecification {
        let mut reader = Decompressor::new(reader, 4096 * 1000);
        bincode::serde::decode_from_std_read(&mut reader, bincode::config::standard()).unwrap()
    }
}

fn merge_ranges(ranges: Vec<Range<u64>>) -> Vec<Range<u64>> {
    if ranges.is_empty() {
        return vec![];
    }

    // First, sort the ranges by their start value
    let mut ranges = ranges;
    ranges.sort_by_key(|r| r.start);

    let mut merged = Vec::new();
    let mut current_range = ranges[0].clone();

    for range in ranges.into_iter().skip(1) {
        if range.start <= current_range.end {
            // Overlapping ranges, merge them
            current_range.end = current_range.end.max(range.end);
        } else {
            // Non-overlapping range, push the current range and start a new one
            merged.push(current_range);
            current_range = range;
        }
    }
    // Push the last range
    merged.push(current_range);

    merged
}

// TODO rework ProcessorSpecificationError to use reports like compiler error
#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The symbol id does not map to a symbol.")]
    SymbolUndeclared(JIRSymbolId),
    #[error("The identifier is not mapped to anything.")]
    IdentifierNotMapped(Spanned<StringId>),
    #[error("The symbol exists but its type does not match.")]
    InvalidSymbolType(MOIRSymbolId),
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
    #[error("The subtable could not be resolved to a constructor")]
    UnresolvedSubtable,
    #[error("The token size is not a multiple of 8 bit")]
    TokenSizeNotAMultipleOfEightBit,
    #[error("The temporary allocator ran out of space.")]
    OutOfTempSpace,
}
