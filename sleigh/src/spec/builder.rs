//! The [ProcessorSpecificationBuilder] is used by the compiler to build up the [ProcessorSpecification].
//! This is done, so that the compiler can work on an incomplete specification and do some final validations,
//! before building the final [ProcessorSpecification].
//! The final [ProcessorSpecification] cannot be modified further.
//! TODO: Improve docs

use super::moir::bitrange_register::{
    BitrangeRegisterId,
    MOIRTBitrangeRegister,
};
use super::moir::context::{
    MOIRContext,
    MOIRContextId,
};
use crate::compiler::ast::{
    Spanned,
    SpannedToken,
};
use crate::compiler::build::matcher::{
    Matcher,
    MatcherId,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::{
    InputSpan,
    ParserInput,
    SleighLexerInput,
};
use crate::compiler::report::{
    generate_invalid_symbol_type_error,
    generate_undefined_ident_error,
};
use crate::compiler::tokens::SleighToken;
use crate::compiler::{
    CompilerContext,
    SleighCompilerResult,
};
use crate::spec::builder::transform::MOIRToJIRTransformer;
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceIsDefault,
    MOIRSpaceType,
};
use crate::spec::moir::code::MOIROperation;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::custom_op::{
    CustomOpId,
    MOIRCustomOp,
};
use crate::spec::moir::endianness::MOIREndianness;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
};
use crate::spec::moir::r#macro::MOIRMacro;
use crate::spec::moir::table::MOIRTable;
use crate::spec::moir::token::{
    MOIRToken,
    TokenId,
};
use crate::spec::moir::varnode::{
    MOIRSlice,
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::reporting::{
    generate_alignment_already_specified_report,
    generate_endianness_already_specified_report,
    generate_identifier_exists_report,
};
use crate::spec::typed_index_table::TypedIndexTable;
use crate::spec::{
    Ident,
    MacroId,
    ProcessorSpecification,
    ProcessorSpecificationError,
    SpaceId,
    TableId,
};
use chumsky::input::{
    Checkpoint,
    Cursor,
    Input,
};
use chumsky::inspector::Inspector;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::{
    Display,
    Formatter,
};
use string_interner::StringInterner;
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::symbol::Symbol;
use strum_macros::Display;
use tracing::debug;

mod transform;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
/// StringId given out by interner. The interner is used by the compiler
/// to simplify string comparisons by mapping ids to strings.
pub struct StringId(usize);

impl Display for StringId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Display, PartialEq, Eq, Hash)]
pub enum MOIRSymbolId {
    /// The identifier is not defined yet and can be used to define a symbol.
    Undefined,
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
/// A transparent helper struct that contains all the symbol tables to make the builder more manageable.
pub(super) struct MOIRSymbolTables {
    pub(super) macros: TypedIndexTable<MacroId, MOIRMacro>,
    pub(super) spaces: TypedIndexTable<SpaceId, MOIRSpace>,
    pub(super) tables: TypedIndexTable<TableId, MOIRTable>,
    pub(super) constructors: TypedIndexTable<ConstructorId, MOIRConstructor>,
    pub(super) varnodes: TypedIndexTable<VarNodeId, MOIRVarNode>,
    pub(super) tokens: TypedIndexTable<TokenId, MOIRToken>,
    pub(super) fields: TypedIndexTable<FieldId, MOIRField>,
    pub(super) custom_ops: TypedIndexTable<CustomOpId, MOIRCustomOp>,
    // bitrange registers are treated more like placeholders for the corresponding pcode operations to realize access
    pub(super) bitrange_registers: TypedIndexTable<BitrangeRegisterId, MOIRTBitrangeRegister>,
}

#[derive(Debug)]
pub struct ProcessorSpecificationBuilder {
    interner: StringInterner<StringBackend>,

    /// This hashmap simply keeps track of which string id is associated with which builtin operation.
    /// It is used to look up the operation type when resolving invocations.
    builtin_ops: HashMap<StringId, MOIROperation>,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,

    /// Maps the identifier ids to corresponding strings.
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    /// Contains a table for each symbol type. A [Ident] maps to a [MOIRSymbolId], which in turn maps to the
    /// corresponding [TypedIndexTable] with a table-specific id type.
    symbol_tables: MOIRSymbolTables,
    /// The endianness of the processor. It must be defined.
    endianness: Option<Spanned<MOIREndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    alignment: Option<Spanned<u16>>,
    /// Context defined on top of existing varnodes containing context fields (e.g. a dedicated
    /// `contextreg` varnode containing a field `longmode=(0,1)` for x86).
    context: SlotMap<MOIRContextId, MOIRContext>,

    matchers: SlotMap<MatcherId, Matcher>,

    /// Maps a string identifier to the corresponding internal [Ident].
    /// This is useful for the compiler, when an identifier string references another symbol.
    /// This will need to be resolved to the correct [Ident], so that the PSB can then
    /// use the internal identifiers, instead of the strings.
    string_to_ident_mapping: HashMap<StringId, Ident>,

    root_table: Spanned<StringId>,
    /// The default space. It has to be set at one point, and only once.
    /// The Ident is spanned with the location of the first definition.
    default_space: Option<Spanned<SpaceId>>,
    temp_space: SpaceId,

    /// The `_` character has special meaning as a placeholder in e.g. stringlists.
    /// The id of the character is stored here for quick comparison.
    placeholder_ident: StringId,
}

impl ProcessorSpecificationBuilder {
    pub(crate) fn interner(&self) -> &StringInterner<StringBackend> {
        &self.interner
    }

    /// Define the endianness. If the endianness was previously defined, an error report is generated.
    pub(crate) fn define_endianness(
        &mut self,
        endianness: Spanned<MOIREndianness>,
    ) -> Result<(), SleighCompilerError> {
        if let Some(old_endianness) = &self.endianness {
            Err(generate_endianness_already_specified_report(old_endianness, endianness.into_span()))
        } else {
            self.endianness = Some(endianness.transmute());
            Ok(())
        }
    }

    /// Define the alignment. If the alignment was previously defined, an error report is generated.
    pub(crate) fn define_alignment(
        &mut self,
        alignment: Spanned<u16>,
    ) -> SleighCompilerResult {
        if let Some(old_alignment) = &self.alignment {
            Err(generate_alignment_already_specified_report(old_alignment, alignment.into_span()))
        } else {
            self.alignment = Some(alignment.transmute());
            Ok(())
        }
    }

    pub(crate) fn declare_custom_op(
        &mut self,
        ident: Spanned<StringId>,
    ) -> Result<Ident, SleighCompilerError> {
        let symbol_id = self.declare_symbol(ident)?;
        let custom_op_id = self.symbol_tables.custom_ops.insert(MOIRCustomOp::Declared);
        self.define_symbol(symbol_id, MOIRSymbolId::CustomOp(custom_op_id))?;
        Ok(symbol_id)
    }

    /// Define a new macro identified by `ident`.
    /// If `ident` was already defined previously,
    /// an error report is returned, describing
    /// where the definition occurred.
    pub(crate) fn define_macro(
        &mut self,
        ident: Spanned<StringId>,
        moir_macro: MOIRMacro,
    ) -> SleighCompilerResult<Ident> {
        let symbol_id = self.declare_symbol(ident)?;
        let macro_id = self.symbol_tables.macros.insert(moir_macro);
        self.define_symbol(symbol_id, MOIRSymbolId::Macro(macro_id))?;
        Ok(symbol_id)
    }

    /// Define a constructor and get the id in the symbol table. Since constructors are not associated
    /// directly with a symbol, this id is the only reference to the constructor, and needs to be stored
    /// somewhere for further access. Usually, the corresponding matcher will receive the id of the
    /// constructor in order to look it up in order to construct an instruction if the matcher matched
    /// the input.
    pub fn define_constructor(
        &mut self,
        constructor: MOIRConstructor,
    ) -> SleighCompilerResult<ConstructorId> {
        Ok(self.symbol_tables.constructors.insert(constructor))
    }

    pub(crate) fn define_space(
        &mut self,
        ident: Spanned<StringId>,
        space: MOIRSpace,
        default: MOIRSpaceIsDefault,
    ) -> SleighCompilerResult<Ident> {
        if matches!(default, MOIRSpaceIsDefault::Default) && self.default_space.is_some() {
            todo!("Generate report that default space is already set")
        }
        // We take the span of the identifier to later add it to the symbol so we can find out where
        // the default was previously defined.
        let span = ident.span().clone();
        let symbol_id = self.declare_symbol(ident)?;
        let space_id = self.symbol_tables.spaces.insert(space);
        self.define_symbol(symbol_id, MOIRSymbolId::Space(space_id))?;
        if matches!(default, MOIRSpaceIsDefault::Default) {
            self.default_space = Some(Spanned::new(space_id, span));
        }
        Ok(symbol_id)
    }

    pub(crate) fn define_varnode(
        &mut self,
        ident: Spanned<StringId>,
        varnode: MOIRVarNode,
    ) -> Result<Ident, SleighCompilerError> {
        let symbol_id = self.declare_symbol(ident)?;
        let varnode_id = self.symbol_tables.varnodes.insert(varnode);
        self.define_symbol(symbol_id, MOIRSymbolId::VarNode(varnode_id))?;
        Ok(symbol_id)
    }

    /// Defines a new temporary varnode. Each instruction has its own temporaries and
    /// they are only visible in their local scope. This means that for each instruction
    /// the temporaries will overlap.
    pub fn define_temp_varnode(
        &mut self,
        offset: u64,
        size: MOIRSize,
    ) -> SleighCompilerResult<VarNodeId> {
        Ok(self.symbol_tables.varnodes.insert(MOIRVarNode {
            space: self.temp_space,
            offset,
            size,
        }))
    }

    pub(crate) fn define_bitrange_register(
        &mut self,
        ident: Spanned<StringId>,
        source_varnode: Spanned<StringId>,
        offset: u64,
        size: u64,
    ) -> Result<BitrangeRegisterId, SleighCompilerError> {
        // validate that source_varnode is valid
        // TODO: Generate error reports instead of normal errors.
        let source_varnode_id = self
            .get_symbol_and_ident(&source_varnode)
            .ok_or(ProcessorSpecificationError::IdentifierNotMapped(source_varnode))
            .and_then(|(symbol, _)| {
                if let MOIRSymbolId::VarNode(vnid) = symbol {
                    Ok(vnid)
                } else {
                    Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))
                }
            })?;

        let moir_bitrange_register = MOIRTBitrangeRegister {
            source_varnode: *source_varnode_id,
            offset,
            size,
        };

        // declare and define symbol
        let symbol_id = self.declare_symbol(ident)?;
        let bitrange_register_id = self
            .symbol_tables
            .bitrange_registers
            .insert(moir_bitrange_register);
        self.define_symbol(symbol_id, MOIRSymbolId::BitrangeRegister(bitrange_register_id))?;
        Ok(bitrange_register_id)
    }

    pub(crate) fn define_token(
        &mut self,
        ident: Spanned<StringId>,
        token: MOIRToken,
    ) -> Result<TokenId, SleighCompilerError> {
        let symbol_id = self.declare_symbol(ident)?;
        let token_id = self.symbol_tables.tokens.insert(token);
        self.define_symbol(symbol_id, MOIRSymbolId::Token(token_id))?;
        Ok(token_id)
    }

    pub(crate) fn define_field(
        &mut self,
        ident: Spanned<StringId>,
        field: MOIRField,
    ) -> Result<Ident, SleighCompilerError> {
        let symbol_id = self.declare_symbol(ident)?;
        let field_id = self.symbol_tables.fields.insert(field);
        self.define_symbol(symbol_id, MOIRSymbolId::Field(field_id))?;
        Ok(symbol_id)
    }

    pub(crate) fn attach_variables(
        &mut self,
        fields: Vec<Spanned<StringId>>,
        varnodes: Vec<Spanned<StringId>>,
    ) -> Result<(), SleighCompilerError> {
        let varnode_ids: Result<Vec<_>, _> = varnodes
            .into_iter()
            .filter_map(|vsid| {
                if self.is_placeholder_ident(*vsid.value()).ok()? {
                    // skip placeholder
                    return None;
                }
                Some(
                    self.get_symbol_and_ident(&vsid)
                        .ok_or(ProcessorSpecificationError::IdentifierNotMapped(vsid))
                        .and_then(|(symbol, _)| {
                            if let MOIRSymbolId::VarNode(vnid) = symbol {
                                Ok(*vnid)
                            } else {
                                Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))
                            }
                        }),
                )
            })
            .collect();

        let varnode_ids = varnode_ids?;

        for field in fields.into_iter() {
            let (symbol, _) = self
                .get_symbol_and_ident(&field)
                .ok_or(ProcessorSpecificationError::IdentifierNotMapped(field))?;

            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_symbol_id_mut(*id)
                    .expect("If the symbol is mapped there has to be an entry in the symbol table")
                    .variables
                    .extend(varnode_ids.iter());
            } else {
                Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?
            }
        }

        Ok(())
    }

    pub(crate) fn attach_values(
        &mut self,
        fields: Vec<Spanned<StringId>>,
        values: Vec<Spanned<u64>>,
    ) -> Result<(), SleighCompilerError> {
        for field in fields.into_iter() {
            let (symbol, _) = self
                .get_symbol_and_ident(&field)
                .ok_or(SleighCompilerError::IdentifierNotMapped(field))?;
            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_symbol_id_mut(*id)
                    .expect("If the symbol is mapped there has to be an entry in the symbol table")
                    .values
                    .extend(values.iter().map(Spanned::value));
            } else {
                Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?
            }
        }

        Ok(())
    }

    pub(crate) fn attach_names(
        &mut self,
        fields: Vec<Spanned<StringId>>,
        strings: Vec<Spanned<StringId>>,
    ) -> Result<(), SleighCompilerError> {
        for field in fields.into_iter() {
            let (symbol, _) = self
                .get_symbol_and_ident(&field)
                .ok_or(ProcessorSpecificationError::IdentifierNotMapped(field))?;
            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_symbol_id_mut(*id)
                    .expect("If the symbol is mapped there has to be an entry in the symbol table")
                    .strings
                    .extend(strings.iter().map(Spanned::value));
            } else {
                Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?
            }
        }

        Ok(())
    }

    pub(crate) fn define_context(
        &mut self,
        varnode_ident: Spanned<StringId>,
    ) -> SleighCompilerResult<(MOIRContextId, u64)> {
        let varnode_id = self.varnode_id_by_ident(&varnode_ident)?;
        let moir_context = MOIRContext { varnode: varnode_id };
        let varnode_bits = self
            .varnode_by_symbol_id(varnode_id)
            .expect("We just verified it")
            .size
            * 8;

        let context_id = self.context.insert(moir_context);
        Ok((context_id, varnode_bits as u64))
    }

    pub(crate) fn modify_root_table(&mut self) -> SleighCompilerResult<&mut MOIRTable> {
        self.modify_table(self.root_table.clone())
    }

    pub(crate) fn modify_table(
        &mut self,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<&mut MOIRTable> {
        let table_id = if let Some((symbol, _)) = self.get_symbol_and_ident(&ident) {
            // Symbol is defined. Do stuff with it.
            if let MOIRSymbolId::Table(table_id) = symbol {
                *table_id
            } else {
                todo!("Generate error report")
            }
        } else {
            // Symbol needs to be defined first.
            let symbol_id = self.declare_symbol(ident)?;
            let table_id = self.symbol_tables.tables.insert(MOIRTable::default());
            self.define_symbol(symbol_id, MOIRSymbolId::Table(table_id))?;
            table_id
        };
        Ok(self
            .symbol_tables
            .tables
            .get_mut(table_id)
            .expect("We either just inserted the new table, or fetched an existing one."))
    }

    pub(crate) fn get_symbol_and_ident(
        &self,
        ident: &Spanned<StringId>,
    ) -> Option<(&MOIRSymbolId, Ident)> {
        self.string_to_ident_mapping.get(ident).map(|ident| {
            (
                self.symbol_ids
                    .get(*ident)
                    .expect("If the ident is mapped, then there has to exist a symbol."),
                *ident,
            )
        })
    }

    /// Get the symbol associated with ident. Returns a detailed error report if no such symbol exists.
    pub(crate) fn get_symbol(
        &self,
        ident: &Spanned<StringId>,
    ) -> SleighCompilerResult<&MOIRSymbolId> {
        trace_build!("Resolving symbol {}", self.resolve_string_id(*ident.value()));
        self.string_to_ident_mapping
            .get(ident)
            .map(|ident| {
                self.symbol_ids
                    .get(*ident)
                    .expect("If the ident is mapped, then there has to exist a symbol")
            })
            .ok_or(generate_undefined_ident_error(Spanned::new(
                self.resolve_string_id(ident.value().clone()),
                ident.span().clone(),
            )))
    }

    /// Attempts to declare a symbol with the provided identifier.
    /// If the identifier is already occupied, an error is returned.
    /// Otherwise, a reference to the newly allocated symbol slot and its id is returned.
    fn declare_symbol(
        &mut self,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<Ident> {
        match self.string_to_ident_mapping.entry(*ident) {
            // The identifier is already declared, and we do not allow re-declaring a symbol.
            Entry::Occupied(entry) => {
                let name = self
                    .symbol_strings
                    .get(entry.get())
                    .expect("The symbol string must exist");
                let name_str = self.resolve_string_id(**name);
                Err(generate_identifier_exists_report(name_str, name.span().clone(), ident.into_span()))
            }
            Entry::Vacant(entry) => {
                // No symbol has been declared yet. We can now insert an empty declaration.
                let id = self.symbol_ids.insert(MOIRSymbolId::Undefined);
                self.symbol_strings.insert(id, ident);
                entry.insert(id);
                Ok(id)
            }
        }
    }

    /// Helper function to define a symbol after declaring it.
    fn define_symbol(
        &mut self,
        symbol_id: Ident,
        symbol: MOIRSymbolId,
    ) -> SleighCompilerResult {
        *self
            .symbol_ids
            .get_mut(symbol_id)
            .ok_or(SleighCompilerError::SymbolUndeclared)? = symbol;
        Ok(())
    }

    pub(crate) fn space_id_by_ident(
        &self,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<SpaceId> {
        self.string_to_ident_mapping
            .get(ident.value())
            .and_then(|id| {
                self.symbol_ids.get(*id).and_then(|id| match id {
                    MOIRSymbolId::Space(space_id) => Some(*space_id),
                    _ => None,
                })
            })
            // TODO: Produce error report instead
            .ok_or(SleighCompilerError::SymbolUndeclared)
    }

    pub fn space_by_symbol_id(
        &self,
        symbol_id: SpaceId,
    ) -> Option<&MOIRSpace> {
        self.symbol_tables.spaces.get(symbol_id)
    }

    /// Get the varnode id referred to by `ident`. If `ident` refers to an identifier that is not a varnode,
    /// or if the identifier is undefined, an error report is generated.
    pub(crate) fn varnode_id_by_ident(
        &self,
        ident: &Spanned<StringId>,
    ) -> SleighCompilerResult<VarNodeId> {
        let symbol_reference = ident.clone().map(|id| self.resolve_string_id(id));
        self.get_symbol_and_ident(ident)
            .ok_or_else(|| generate_undefined_ident_error(symbol_reference.clone()))
            .and_then(|(symbol, id)| match symbol {
                MOIRSymbolId::VarNode(vnid) => Ok(*vnid),
                _ => {
                    let defined_symbol = self
                        .symbol_strings
                        .get(&id)
                        .expect("If the symbol exists, there must be a string associated with it.")
                        .span()
                        .clone();
                    Err(generate_invalid_symbol_type_error(defined_symbol, symbol_reference, "VarNode", *symbol))?
                }
            })
    }

    /// Get the varnode pointed to by `id`
    pub(crate) fn varnode_by_symbol_id(
        &self,
        id: VarNodeId,
    ) -> Option<&MOIRVarNode> {
        self.symbol_tables.varnodes.get(id)
    }

    /// Get the varnode slice referred to by `ident`. If `ident` refers to an identifier that is not a varnode,
    /// or if the identifier is undefined, an error report is generated.
    pub(crate) fn varnode_slice_by_ident(
        &self,
        ident: &Spanned<StringId>,
    ) -> SleighCompilerResult<MOIRSlice> {
        self.varnode_id_by_ident(ident).map(|vnid| {
            let varnode = self
                .varnode_by_symbol_id(vnid)
                .expect("This must exist since we just got its id.");
            MOIRSlice::varnode(vnid, 0, varnode.size)
        })
    }

    pub(crate) fn ident_by_symbol(
        &self,
        symbol: &MOIRSymbolId,
    ) -> Option<&Spanned<StringId>> {
        self.symbol_ids
            .iter()
            .find_map(|(k, v)| if v == symbol { self.symbol_strings.get(&k) } else { None })
    }

    // TODO put into macro
    pub(crate) fn field_by_symbol_id(
        &self,
        id: FieldId,
    ) -> Option<&MOIRField> {
        self.symbol_tables.fields.get(id)
    }

    pub(crate) fn field_by_symbol_id_mut(
        &mut self,
        id: FieldId,
    ) -> Option<&mut MOIRField> {
        self.symbol_tables.fields.get_mut(id)
    }

    pub(crate) fn table_by_symbol_id(
        &self,
        id: TableId,
    ) -> Option<&MOIRTable> {
        self.symbol_tables.tables.get(id)
    }

    pub(crate) fn is_placeholder_ident(
        &self,
        ident: StringId,
    ) -> Result<bool, ProcessorSpecificationError> {
        Ok(self.placeholder_ident == ident)
    }

    pub(crate) fn resolve_string_id(
        &self,
        ident: StringId,
    ) -> &str {
        self.string(ident)
            .expect("The string must have been interned if we gave out an id")
    }

    /// Resolve the space to its id if a string ident is given, and otherwise return the default space.
    pub fn resolve_space(
        &mut self,
        space: Option<Spanned<StringId>>,
    ) -> SleighCompilerResult<SpaceId> {
        let space_id = space
            .map(|id| self.space_id_by_ident(id))
            .transpose()?
            .or_else(|| {
                if let Some(default) = &self.default_space {
                    Some(*default.value())
                } else {
                    None
                }
            })
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error for missing default") })?;

        Ok(space_id)
    }

    pub fn builtin_op_by_ident(
        &self,
        ident: &Spanned<StringId>,
    ) -> Option<MOIROperation> {
        self.builtin_ops.get(ident.value()).cloned()
    }
}

impl Default for ProcessorSpecificationBuilder {
    fn default() -> Self {
        let mut interner = StringInterner::new();
        let placeholder_ident = interner.intern("_");
        let epsilon = interner.intern("epsilon");
        let const_space = interner.intern("const");
        let root_table = Spanned::new(interner.intern("instruction"), InputSpan::default());
        let mut symbol_tables = MOIRSymbolTables::default();
        let temp_space = symbol_tables.spaces.insert(MOIRSpace {
            r#type: MOIRSpaceType::Register,
            address_size: 8,
            wordsize: 1,
        });
        let mut slf = Self {
            interner,
            builtin_ops: Default::default(),
            symbol_ids: Default::default(),
            symbol_strings: Default::default(),
            symbol_tables,
            endianness: None,
            alignment: None,
            context: Default::default(),
            matchers: Default::default(),
            string_to_ident_mapping: Default::default(),
            root_table,
            default_space: None,
            temp_space,
            placeholder_ident,
        };
        use MOIROperation::*;
        let builtins = &mut slf.builtin_ops;
        builtins.insert(slf.interner.intern("popcount"), PopCount);
        builtins.insert(slf.interner.intern("lzcount"), LZCount);
        builtins.insert(slf.interner.intern("zext"), ZeroExtend);
        builtins.insert(slf.interner.intern("sext"), SignExtend);
        builtins.insert(slf.interner.intern("carry"), Carry);
        builtins.insert(slf.interner.intern("scarry"), SignCarry);
        builtins.insert(slf.interner.intern("sborrow"), SignBorrow);
        builtins.insert(slf.interner.intern("nan"), Nan);
        builtins.insert(slf.interner.intern("abs"), Abs);
        builtins.insert(slf.interner.intern("sqrt"), Sqrt);
        builtins.insert(slf.interner.intern("int2float"), IntToFloat);
        builtins.insert(slf.interner.intern("float2float"), FloatToFloat);
        builtins.insert(slf.interner.intern("trunc"), Trunc);
        builtins.insert(slf.interner.intern("ceil"), Ceil);
        builtins.insert(slf.interner.intern("floor"), Floor);
        builtins.insert(slf.interner.intern("round"), Round);
        // Call modify once to create the table.
        slf.modify_root_table()
            .expect("This may not error at this point");
        // TODO: Define the special varnodes in a special address space.
        slf.define_space(
            Spanned::new(const_space, InputSpan::default()),
            MOIRSpace {
                r#type: MOIRSpaceType::Const,
                address_size: 0,
                wordsize: 0,
            },
            MOIRSpaceIsDefault::NotDefault,
        )
        .expect("This may not error at this point");
        slf.define_varnode(Spanned::new(epsilon, InputSpan::default()), MOIRVarNode {
            space: SpaceId::from(0),
            offset: 0,
            size: 0,
        })
        .expect("This may not error at this point");
        slf
    }
}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(mut self) -> SleighCompilerResult<ProcessorSpecification> {
        debug!("Finalizing spec:");
        debug!("Found {} symbols", self.symbol_ids.len());
        debug!("Found {} tables", self.symbol_tables.tables.len());
        debug!("Found {} macros", self.symbol_tables.macros.len());
        debug!("Found {} tokens", self.symbol_tables.tokens.len());
        debug!("Found {} fields", self.symbol_tables.fields.len());
        debug!("Found {} spaces", self.symbol_tables.spaces.len());
        debug!("Found {} global varnodes", self.symbol_tables.varnodes.len());
        debug!("Found {} custom operations", self.symbol_tables.custom_ops.len());
        let root_table = self.table_id_by_ident(&self.root_table)?;
        let context = MOIRToJIRTransformer::new(
            self.symbol_ids,
            self.symbol_strings,
            self.symbol_tables,
            self.string_to_ident_mapping,
        );
        let jir = context.transform()?;
        // TODO: Generate more helpful error reports during final build step
        let spec = ProcessorSpecification {
            // The endianness must be defined, so we return an error if it is undefined at this point.
            interner: self.interner,
            symbol_ids: jir.symbol_ids,
            symbol_strings: jir.symbol_strings,
            symbol_tables: jir.symbol_tables,
            string_to_ident_mapping: jir.string_to_ident_mapping,
            root_table,
            default_space: self
                .default_space
                .ok_or(ProcessorSpecificationError::MissingDefaultSpace)?
                .into_value(),
            temp_space: self.temp_space,
            endianness: self
                .endianness
                .ok_or(ProcessorSpecificationError::MissingEndiannessDefinition)?
                .into_value(),
            // The default alignment is 1
            alignment: self.alignment.map(Spanned::into_value).unwrap_or(1),
        };
        Ok(spec)
    }
}
pub trait CustomInterner {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId;
    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId>;

    /// Resolve a string id to the corresponding interned string.
    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str>;
}
impl CustomInterner for StringInterner<StringBackend> {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId {
        StringId(self.get_or_intern(string).to_usize())
    }

    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId> {
        self.get(string).map(|id| StringId(id.to_usize()))
    }

    /// Resolve a string id to the corresponding interned string.
    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.resolve(<<StringBackend as Backend>::Symbol as Symbol>::try_from_usize(string_id.0)?)
    }
}

impl CompilerContext for ProcessorSpecificationBuilder {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: T,
    ) -> StringId {
        self.interner.intern(string)
    }

    fn lookup<T: AsRef<str>>(
        &self,
        string: T,
    ) -> Option<StringId> {
        self.interner.lookup(string)
    }

    /// Resolve a string id to the corresponding interned string.
    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.interner.string(string_id)
    }
}

impl<'src, I: Input<'src>> Inspector<'src, I> for ProcessorSpecificationBuilder {
    type Checkpoint = ();

    fn on_token(
        &mut self,
        token: &I::Token,
    ) {
    }

    fn on_save<'parse>(
        &self,
        cursor: &Cursor<'src, 'parse, I>,
    ) -> Self::Checkpoint {
        ()
    }

    fn on_rewind<'parse>(
        &mut self,
        marker: &Checkpoint<'src, 'parse, I, Self::Checkpoint>,
    ) {
    }
}

macro_rules! impl_get_symbol_struct {
    ($func_name:ident, $func_name_mut:ident, $table:ident, $symbol:ty, $variant:pat => $id:expr) => {
        impl ProcessorSpecificationBuilder {
            pub(crate) fn $func_name(
                &self,
                ident: &Spanned<StringId>,
            ) -> Option<&$symbol> {
                let (symbol, _) = self.get_symbol_and_ident(ident)?;
                match symbol {
                    $variant => self.symbol_tables.$table.get(*$id),
                    _ => None,
                }
            }

            pub(crate) fn $func_name_mut(
                &mut self,
                ident: &Spanned<StringId>,
            ) -> Option<&mut $symbol> {
                let (symbol, _) = self.get_symbol_and_ident(ident)?;
                match symbol {
                    $variant => self.symbol_tables.$table.get_mut(*$id),
                    _ => None,
                }
            }
        }
    };
}

macro_rules! impl_get_symbol_id {
    ($func_name:ident, $symbol_id:ty, $variant:pat => $id:expr) => {
        impl ProcessorSpecificationBuilder {
            /// Get the symbol id referred to by `ident`. If `ident` refers to an identifier that has the wrong type,
            /// or if the identifier is undefined, an error report is generated.
            pub fn $func_name(
                &self,
                ident: &Spanned<StringId>,
            ) -> SleighCompilerResult<$symbol_id> {
                let symbol_reference = ident.clone().map(|id| self.resolve_string_id(id));
                self.get_symbol_and_ident(ident)
                    .ok_or_else(|| generate_undefined_ident_error(symbol_reference.clone()))
                    .and_then(|(symbol, id)| match symbol {
                        $variant => Ok(*$id),
                        _ => {
                            let defined_symbol = self
                                .symbol_strings
                                .get(&id)
                                .expect("If the symbol exists, there must be a string associated with it.")
                                .span()
                                .clone();
                            Err(generate_invalid_symbol_type_error(
                                defined_symbol,
                                symbol_reference,
                                stringify!($symbol_id),
                                *symbol,
                            ))?
                        }
                    })
            }
        }
    };
}

impl_get_symbol_struct! { macro_by_ident, macro_by_ident_mut, macros, MOIRMacro, MOIRSymbolId::Macro(id) => id}
impl_get_symbol_struct! { space_by_ident, space_by_ident_mut, spaces, MOIRSpace, MOIRSymbolId::Space(id) => id}
impl_get_symbol_struct! { table_by_ident, table_by_ident_mut, tables, MOIRTable, MOIRSymbolId::Table(id) => id}
impl_get_symbol_struct! { varnode_by_ident, varnode_by_ident_mut, varnodes, MOIRVarNode, MOIRSymbolId::VarNode(id) => id}
impl_get_symbol_struct! { token_by_ident, token_by_ident_mut, tokens, MOIRToken, MOIRSymbolId::Token(id) => id}
impl_get_symbol_struct! { field_by_ident, field_by_ident_mut, fields, MOIRField, MOIRSymbolId::Field(id) => id}
impl_get_symbol_struct! { custom_op_by_ident, custom_op_by_ident_mut, custom_ops, MOIRCustomOp, MOIRSymbolId::CustomOp(id) => id}
impl_get_symbol_struct! { bitrange_register_by_ident, bitrange_register_by_ident_mut, bitrange_registers, MOIRTBitrangeRegister, MOIRSymbolId::BitrangeRegister(id) => id}

impl_get_symbol_id! { field_id_by_ident, FieldId, MOIRSymbolId::Field(id) => id}
impl_get_symbol_id! { custom_op_id_by_ident, CustomOpId, MOIRSymbolId::CustomOp(id) => id}
impl_get_symbol_id! { table_id_by_ident, TableId, MOIRSymbolId::Table(id) => id}
