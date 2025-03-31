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
    ContextId,
    MOIRContext,
};
use crate::compiler::InternalCompilerResult;
use crate::compiler::build::trace_build;
use crate::compiler::error::{
    DuplicateDefineType,
    DuplicatedDefine,
    DuplicatedIdentifier,
    InternalError,
    InternalErrorType,
    InvalidSymbolType,
    UndefinedIdentifier,
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
use crate::spec::{
    MacroId,
    ProcessorSpecification,
    ProcessorSpecificationError,
    SpaceId,
    TableId,
};
use fnv::FnvHashMap;
use mogi_sleigh_parser::lexer::InputSpan;
use mogi_sleigh_parser::{
    ParserContext,
    SleighInterner,
    Spanned,
    StringId,
};
use mogi_utilities::index_map::{
    MapKey,
    PrimaryMap,
    SecondaryMap,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::OpenOptions;
use std::io::{
    BufWriter,
    Write,
};
use std::path::Path;
use strum_macros::Display;
use tracing::debug;

mod transform;

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
    pub(super) macros: PrimaryMap<MacroId, MOIRMacro>,
    pub(super) spaces: PrimaryMap<SpaceId, MOIRSpace>,
    pub(super) tables: PrimaryMap<TableId, MOIRTable>,
    pub(super) constructors: PrimaryMap<ConstructorId, MOIRConstructor>,
    pub(super) varnodes: PrimaryMap<VarNodeId, MOIRVarNode>,
    pub(super) tokens: PrimaryMap<TokenId, MOIRToken>,
    pub(super) fields: PrimaryMap<FieldId, MOIRField>,
    pub(super) custom_ops: PrimaryMap<CustomOpId, MOIRCustomOp>,
    // bitrange registers are treated more like placeholders for the corresponding pcode operations to realize access
    pub(super) bitrange_registers: PrimaryMap<BitrangeRegisterId, MOIRTBitrangeRegister>,
}

#[derive(Debug)]
pub struct ProcessorSpecificationBuilder {
    interner: SleighInterner,

    /// This hashmap simply keeps track of which string id is associated with which builtin operation.
    /// It is used to look up the operation type when resolving invocations.
    builtin_ops: HashMap<StringId, MOIROperation>,

    /// The main symbol map of the specification.
    /// Each string id points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    symbol_ids: FnvHashMap<StringId, MOIRSymbolId>,

    /// Inverse of [Self::symbol_ids] with spans.
    /// Each symbol id points to the spanned string id it was defined with. This
    /// is mainly useful for debugging and error reporting purposes.
    symbol_strings: FnvHashMap<MOIRSymbolId, Spanned<StringId>>,

    /// Contains a table for each symbol type. A [Ident] maps to a [MOIRSymbolId], which in turn maps to the
    /// corresponding [TypedIndexTable] with a table-specific id type.
    symbol_tables: MOIRSymbolTables,
    /// The endianness of the processor. It must be defined.
    endianness: Option<Spanned<MOIREndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    alignment: Option<Spanned<u16>>,
    /// Context defined on top of existing varnodes containing context fields (e.g. a dedicated
    /// `contextreg` varnode containing a field `longmode=(0,1)` for x86).
    context: PrimaryMap<ContextId, MOIRContext>,

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
    pub(crate) fn interner(&self) -> &SleighInterner {
        &self.interner
    }

    pub(crate) fn interner_mut(&mut self) -> &mut SleighInterner {
        &mut self.interner
    }

    /// Define the endianness. If the endianness was previously defined, an error report is generated.
    pub(crate) fn define_endianness(
        &mut self,
        endianness: Spanned<MOIREndianness>,
    ) -> InternalCompilerResult {
        if let Some(old_endianness) = &self.endianness {
            let e = DuplicatedDefine {
                dupe_type: DuplicateDefineType::Endianness,
            };
            let mut spans = HashMap::new();
            spans.insert("first", old_endianness.span().clone());
            spans.insert("second", endianness.into_span());
            Err(InternalError::with_spans(e.into(), spans))
        } else {
            self.endianness = Some(endianness.transmute());
            Ok(())
        }
    }

    /// Define the alignment. If the alignment was previously defined, an error report is generated.
    pub(crate) fn define_alignment(
        &mut self,
        alignment: Spanned<u16>,
    ) -> InternalCompilerResult {
        if let Some(old_alignment) = &self.alignment {
            let e = DuplicatedDefine {
                dupe_type: DuplicateDefineType::Alignment,
            };
            let mut spans = HashMap::new();
            spans.insert("first", old_alignment.span().clone());
            spans.insert("second", alignment.into_span());
            Err(InternalError::with_spans(e.into(), spans))
        } else {
            self.alignment = Some(alignment.transmute());
            Ok(())
        }
    }

    pub(crate) fn declare_custom_op(
        &mut self,
        ident: Spanned<StringId>,
    ) -> InternalCompilerResult {
        let custom_op_id = self
            .symbol_tables
            .custom_ops
            .insert(MOIRCustomOp::Declared { name: *ident.value() });
        self.define_symbol(ident, MOIRSymbolId::CustomOp(custom_op_id))?;
        Ok(())
    }

    /// Define a new macro identified by `ident`.
    /// If `ident` was already defined previously,
    /// an error report is returned, describing
    /// where the definition occurred.
    pub(crate) fn define_macro(
        &mut self,
        ident: Spanned<StringId>,
        moir_macro: MOIRMacro,
    ) -> InternalCompilerResult {
        let macro_id = self.symbol_tables.macros.insert(moir_macro);
        self.define_symbol(ident, MOIRSymbolId::Macro(macro_id))?;
        Ok(())
    }

    /// Define a constructor and get the id in the symbol table. Since constructors are not associated
    /// directly with a symbol, this id is the only reference to the constructor, and needs to be stored
    /// somewhere for further access. Usually, the corresponding matcher will receive the id of the
    /// constructor in order to look it up in order to construct an instruction if the matcher matched
    /// the input.
    pub fn define_constructor(
        &mut self,
        constructor: MOIRConstructor,
    ) -> InternalCompilerResult<ConstructorId> {
        Ok(self.symbol_tables.constructors.insert(constructor))
    }

    pub(crate) fn next_constructor_id(&self) -> ConstructorId {
        self.symbol_tables.constructors.next_key()
    }

    pub(crate) fn define_space(
        &mut self,
        ident: Spanned<StringId>,
        space: MOIRSpace,
        default: MOIRSpaceIsDefault,
    ) -> InternalCompilerResult {
        if matches!(default, MOIRSpaceIsDefault::Default) && self.default_space.is_some() {
            todo!("Generate report that default space is already set")
        }
        // We take the span of the identifier to later add it to the symbol so we can find out where
        // the default was previously defined.
        let span = ident.span().clone();
        let space_id = self.symbol_tables.spaces.insert(space);
        self.define_symbol(ident, MOIRSymbolId::Space(space_id))?;
        if matches!(default, MOIRSpaceIsDefault::Default) {
            self.default_space = Some(Spanned::new(space_id, span));
        }

        Ok(())
    }

    pub(crate) fn define_varnode(
        &mut self,
        ident: Spanned<StringId>,
        varnode: MOIRVarNode,
    ) -> InternalCompilerResult {
        let varnode_id = self.symbol_tables.varnodes.insert(varnode);
        self.define_symbol(ident, MOIRSymbolId::VarNode(varnode_id))?;

        Ok(())
    }

    /// Defines a new temporary varnode. Each instruction has its own temporaries and
    /// they are only visible in their local scope. This means that for each instruction
    /// the temporaries will overlap.
    pub fn define_temp_varnode(
        &mut self,
        offset: u64,
        size: MOIRSize,
    ) -> InternalCompilerResult<VarNodeId> {
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
    ) -> InternalCompilerResult<BitrangeRegisterId> {
        // validate that source_varnode is valid
        // TODO: Generate error reports instead of normal errors.
        let source_varnode_id = self.get_symbol(&source_varnode).and_then(|symbol| {
            if let MOIRSymbolId::VarNode(vnid) = symbol {
                Ok(vnid)
            } else {
                Err(ProcessorSpecificationError::InvalidSymbolType(*symbol).into())
            }
        })?;

        let moir_bitrange_register = MOIRTBitrangeRegister {
            source_varnode: *source_varnode_id,
            offset,
            size,
        };

        // define symbol
        let bitrange_register_id = self
            .symbol_tables
            .bitrange_registers
            .insert(moir_bitrange_register);
        self.define_symbol(ident, MOIRSymbolId::BitrangeRegister(bitrange_register_id))?;
        Ok(bitrange_register_id)
    }

    pub(crate) fn define_token(
        &mut self,
        ident: Spanned<StringId>,
        token: MOIRToken,
    ) -> Result<TokenId, InternalError> {
        let token_id = self.define_unnamed_token(token)?;
        self.define_symbol(ident, MOIRSymbolId::Token(token_id))?;
        Ok(token_id)
    }

    pub(crate) fn define_unnamed_token(
        &mut self,
        token: MOIRToken,
    ) -> InternalCompilerResult<TokenId> {
        let token_id = self.symbol_tables.tokens.insert(token);
        Ok(token_id)
    }

    pub(crate) fn define_field(
        &mut self,
        ident: Spanned<StringId>,
        field: MOIRField,
    ) -> InternalCompilerResult {
        let field_id = self.symbol_tables.fields.insert(field);
        self.define_symbol(ident, MOIRSymbolId::Field(field_id))?;
        Ok(())
    }

    pub(crate) fn attach_variables(
        &mut self,
        fields: Vec<Spanned<StringId>>,
        varnodes: Vec<Spanned<StringId>>,
    ) -> InternalCompilerResult {
        let varnode_ids: Result<Vec<_>, _> = varnodes
            .into_iter()
            .filter_map(|vsid| {
                if self.is_placeholder_ident(*vsid.value()).ok()? {
                    // skip placeholder
                    return None;
                }
                Some(self.get_symbol(&vsid).and_then(|symbol| {
                    if let MOIRSymbolId::VarNode(vnid) = symbol {
                        Ok(*vnid)
                    } else {
                        Err(ProcessorSpecificationError::InvalidSymbolType(*symbol).into())
                    }
                }))
            })
            .collect();

        let varnode_ids = varnode_ids?;

        for field in fields.into_iter() {
            let symbol = self.get_symbol(&field)?;

            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_id_mut(*id)
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
    ) -> InternalCompilerResult {
        for field in fields.into_iter() {
            let symbol = self.get_symbol(&field)?;
            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_id_mut(*id)
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
    ) -> InternalCompilerResult {
        for field in fields.into_iter() {
            let symbol = self.get_symbol(&field)?;
            if let MOIRSymbolId::Field(id) = symbol {
                self.field_by_id_mut(*id)
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
    ) -> InternalCompilerResult<(ContextId, u64)> {
        let varnode_id = self.varnode_id_by_ident(&varnode_ident)?;
        let moir_context = MOIRContext { varnode: varnode_id };
        let varnode_bits = self.varnode_by_id(varnode_id).size * 8;

        let context_id = self.context.insert(moir_context);
        Ok((context_id, varnode_bits as u64))
    }

    pub(crate) fn get_table_id_or_create(
        &mut self,
        ident: Option<Spanned<StringId>>,
    ) -> InternalCompilerResult<TableId> {
        let ident = ident.unwrap_or(self.root_table.clone());
        if let Ok(symbol) = self.get_symbol(&ident) {
            // Symbol is defined. Do stuff with it.
            if let MOIRSymbolId::Table(table_id) = symbol {
                Ok(*table_id)
            } else {
                todo!("Generate error report")
            }
        } else {
            // Symbol needs to be defined first.
            let table_id = self.symbol_tables.tables.insert(MOIRTable::default());
            self.define_symbol(ident, MOIRSymbolId::Table(table_id))?;
            Ok(table_id)
        }
    }

    /// Modify the provided table. If None is provided, the root table is used.
    pub(crate) fn create_or_modify_table(
        &mut self,
        ident: Option<Spanned<StringId>>,
    ) -> InternalCompilerResult<&mut MOIRTable> {
        let table_id = self.get_table_id_or_create(ident)?;
        Ok(self
            .symbol_tables
            .tables
            .get_mut(table_id)
            .expect("We either just inserted the new table, or fetched an existing one."))
    }

    /// Get the symbol associated with ident. Returns a detailed error report if no such symbol exists.
    pub(crate) fn get_symbol(
        &self,
        ident: &Spanned<StringId>,
    ) -> InternalCompilerResult<&MOIRSymbolId> {
        trace_build!("Resolving symbol {}", self.resolve_string_id(*ident.value()));
        self.symbol_ids.get(ident).ok_or_else(|| {
            let e = UndefinedIdentifier {
                identifier: self.resolve_string_id(ident.value().clone()).to_string(),
            };
            InternalError::with_span(e.into(), "ident", ident.span().clone())
        })
    }

    /// Get the spanned string that was used to define this symbol.
    pub(crate) fn get_string(
        &self,
        symbol: MOIRSymbolId,
    ) -> Spanned<StringId> {
        trace_build!("Resolving symbol {}", symbol);

        *self
            .symbol_strings
            .get(&symbol)
            .expect("Every defined symbol must have come from somewhere in the spec")
    }

    /// Helper function to define a symbol. Return an error if the symbol has been defined
    /// previously.
    fn define_symbol(
        &mut self,
        symbol_id: Spanned<StringId>,
        symbol: MOIRSymbolId,
    ) -> InternalCompilerResult {
        match self.symbol_ids.entry(*symbol_id.value()) {
            Entry::Occupied(_) => {
                let e = DuplicatedIdentifier {
                    name: self.resolve_string_id(*symbol_id.value()).to_string(),
                };

                Err(InternalError::with_span(e.into(), "ident", symbol_id.span().clone()))?;
            }
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(symbol);
                self.symbol_strings.insert(symbol, symbol_id);
            }
        }

        Ok(())
    }

    /// Get the varnode slice referred to by `ident`. If `ident` refers to an identifier that is not a varnode,
    /// or if the identifier is undefined, an error report is generated.
    pub(crate) fn varnode_slice_by_ident(
        &self,
        ident: &Spanned<StringId>,
    ) -> InternalCompilerResult<MOIRSlice> {
        self.varnode_id_by_ident(ident).map(|vnid| {
            let varnode = self.varnode_by_id(vnid);
            MOIRSlice::varnode(vnid, 0, varnode.size)
        })
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
    ) -> InternalCompilerResult<SpaceId> {
        let space_id = space
            .map(|id| self.space_id_by_ident(&id))
            .transpose()?
            .or_else(|| {
                if let Some(default) = &self.default_space {
                    Some(*default.value())
                } else {
                    None
                }
            })
            .ok_or(InternalErrorType::UndeclaredSymbol)?;

        Ok(space_id)
    }

    /// Resolve the table to its id if a string ident is given, and otherwise return the root table.
    pub fn resolve_table(
        &self,
        ident: Option<Spanned<StringId>>,
    ) -> InternalCompilerResult<&MOIRTable> {
        let ident = ident.unwrap_or(self.root_table.clone());
        self.non_root_table_by_ident(&ident)
    }

    /// Resolve the table to its id if a string ident is given, and otherwise return the root table.
    pub fn resolve_table_mut(
        &mut self,
        ident: Option<Spanned<StringId>>,
    ) -> InternalCompilerResult<&mut MOIRTable> {
        let ident = ident.unwrap_or(self.root_table.clone());
        self.non_root_table_by_ident_mut(&ident)
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
        let mut interner = SleighInterner::default();
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
        slf.create_or_modify_table(None)
            .expect("This may not error at this point");
        // TODO: Define the special varnodes in a special address space.
        slf.define_space(
            Spanned::new(const_space, InputSpan::default()),
            MOIRSpace {
                r#type: MOIRSpaceType::Const,
                address_size: 8,
                wordsize: 0,
            },
            MOIRSpaceIsDefault::NotDefault,
        )
        .expect("This may not error at this point");
        slf.define_varnode(
            Spanned::new(epsilon, InputSpan::default()),
            MOIRVarNode {
                space: SpaceId::try_from_index(0).expect("Zero is a valid id"),
                offset: 0,
                size: 0,
            },
        )
        .expect("This may not error at this point");
        slf
    }
}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn dump_symbols_to_file(
        &self,
        path: &Path,
    ) {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .expect("Couldn't open file");
        let mut writer = BufWriter::new(file);

        for (string_id, symbol_id) in self.symbol_ids.iter() {
            let string = self.resolve_string_id(*string_id);
            write!(writer, "{:?} <=> {}\n", symbol_id, string).unwrap();
        }

        for (cid, c) in self.symbol_tables.constructors.items() {
            write!(writer, "{:?} <=> {}\n", cid, c.display_string).unwrap();
        }
    }

    pub fn build(self) -> InternalCompilerResult<ProcessorSpecification> {
        debug!("Finalizing spec:");
        debug!("Found {} symbols", self.symbol_ids.len());
        debug!("Found {} tables", self.symbol_tables.tables.len());
        debug!("Found {} macros", self.symbol_tables.macros.len());
        debug!("Found {} tokens", self.symbol_tables.tokens.len());
        debug!("Found {} context varnodes", self.context.len());
        debug!("Found {} fields", self.symbol_tables.fields.len());
        debug!("Found {} spaces", self.symbol_tables.spaces.len());
        debug!("Found {} global varnodes", self.symbol_tables.varnodes.len());
        debug!("Found {} custom operations", self.symbol_tables.custom_ops.len());

        // self.dump_symbols_to_file(&PathBuf::from("symbol_strings.txt"));

        let root_table = self.table_id_by_ident(&self.root_table)?;
        let context = MOIRToJIRTransformer::new(
            &self.interner,
            self.symbol_ids,
            self.symbol_strings,
            self.symbol_tables,
            &self.context,
            root_table,
        );
        let jir = context.transform()?;
        // TODO: Generate more helpful error reports during final build step
        let spec = ProcessorSpecification {
            // The endianness must be defined, so we return an error if it is undefined at this point.
            interner: self.interner,
            symbol_ids: jir.symbol_ids,
            symbol_strings: jir.symbol_strings,
            symbol_tables: jir.symbol_tables,
            contexts: self
                .context
                .items()
                .map(|(id, context)| (context.varnode, id))
                .collect(),
            varnode_groups: jir.varnode_groups,
            varnode_to_group: jir.varnode_to_group,
            jumplist_cache: jir.jumplist_cache,
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
            lifter_diagnostics: RefCell::new(Default::default()),
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

impl CustomInterner for ProcessorSpecificationBuilder {
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

macro_rules! impl_get_symbol_by_ident {
    ($func_name:ident, $func_name_mut:ident, $table:ident, $symbol:ty, $variant:pat => $id:expr) => {
        impl ProcessorSpecificationBuilder {
            pub fn $func_name(
                &self,
                ident: &Spanned<StringId>,
            ) -> InternalCompilerResult<&$symbol> {
                let symbol = self.get_symbol(ident)?;

                match symbol {
                    $variant => Ok(self
                        .symbol_tables
                        .$table
                        .get(*$id)
                        .expect("If we have an id we must have an instance")),
                    _ => {
                        let ident_str = self.resolve_string_id(*ident.value());
                        let defined = self.get_string(*symbol);

                        let mut spans = HashMap::new();
                        spans.insert("defined", defined.span().clone());
                        spans.insert("reference", ident.span().clone());
                        let e = InvalidSymbolType {
                            symbol_reference: ident_str.to_string(),
                            expected: stringify!($symbol_id).to_string(),
                            found: *symbol,
                        };
                        Err(InternalError::with_spans(e.into(), spans))?
                    }
                }
            }

            pub fn $func_name_mut(
                &mut self,
                ident: &Spanned<StringId>,
            ) -> InternalCompilerResult<&mut $symbol> {
                let symbol = self.get_symbol(ident)?;

                match symbol {
                    $variant => Ok(self
                        .symbol_tables
                        .$table
                        .get_mut(*$id)
                        .expect("If we have an id we must have an instance")),
                    _ => {
                        let ident_str = self.resolve_string_id(*ident.value());
                        let defined = self.get_string(*symbol);

                        let mut spans = HashMap::new();
                        spans.insert("defined", defined.span().clone());
                        spans.insert("reference", ident.span().clone());
                        let e = InvalidSymbolType {
                            symbol_reference: ident_str.to_string(),
                            expected: stringify!($symbol_id).to_string(),
                            found: *symbol,
                        };
                        Err(InternalError::with_spans(e.into(), spans))?
                    }
                }
            }
        }
    };
}

macro_rules! impl_get_symbol_by_id {
    ($func_name:ident, $func_name_mut:ident, $table:ident, $symbol:ty, $symbol_id:ty) => {
        impl ProcessorSpecificationBuilder {
            pub fn $func_name(
                &self,
                id: $symbol_id,
            ) -> &$symbol {
                self.symbol_tables
                    .$table
                    .get(id)
                    .expect("If we have an id we must have an instance")
            }

            pub fn $func_name_mut(
                &mut self,
                id: $symbol_id,
            ) -> &mut $symbol {
                self.symbol_tables
                    .$table
                    .get_mut(id)
                    .expect("If we have an id we must have an instance")
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
            ) -> InternalCompilerResult<$symbol_id> {
                let symbol = self.get_symbol(ident)?;

                match symbol {
                    $variant => Ok(*$id),
                    _ => {
                        let ident_str = self.resolve_string_id(*ident.value());
                        let defined = self.get_string(*symbol);

                        let mut spans = HashMap::new();
                        spans.insert("defined", defined.span().clone());
                        spans.insert("reference", ident.span().clone());
                        let e = InvalidSymbolType {
                            symbol_reference: ident_str.to_string(),
                            expected: stringify!($symbol_id).to_string(),
                            found: *symbol,
                        };
                        Err(InternalError::with_spans(e.into(), spans))?
                    }
                }
            }
        }
    };
}

impl_get_symbol_by_ident! { macro_by_ident, macro_by_ident_mut, macros, MOIRMacro, MOIRSymbolId::Macro(id) => id}
impl_get_symbol_by_ident! { space_by_ident, space_by_ident_mut, spaces, MOIRSpace, MOIRSymbolId::Space(id) => id}
impl_get_symbol_by_ident! { non_root_table_by_ident, non_root_table_by_ident_mut, tables, MOIRTable, MOIRSymbolId::Table(id) => id}
impl_get_symbol_by_ident! { varnode_by_ident, varnode_by_ident_mut, varnodes, MOIRVarNode, MOIRSymbolId::VarNode(id) => id}
impl_get_symbol_by_ident! { token_by_ident, token_by_ident_mut, tokens, MOIRToken, MOIRSymbolId::Token(id) => id}
impl_get_symbol_by_ident! { field_by_ident, field_by_ident_mut, fields, MOIRField, MOIRSymbolId::Field(id) => id}
impl_get_symbol_by_ident! { custom_op_by_ident, custom_op_by_ident_mut, custom_ops, MOIRCustomOp, MOIRSymbolId::CustomOp(id) => id}
impl_get_symbol_by_ident! { bitrange_register_by_ident, bitrange_register_by_ident_mut, bitrange_registers, MOIRTBitrangeRegister, MOIRSymbolId::BitrangeRegister(id) => id}

impl_get_symbol_by_id! { field_by_id, field_by_id_mut, fields, MOIRField, FieldId }
impl_get_symbol_by_id! { table_by_id, table_by_id_mut, tables, MOIRTable, TableId }
impl_get_symbol_by_id! { varnode_by_id, varnode_by_id_mut, varnodes, MOIRVarNode, VarNodeId }
impl_get_symbol_by_id! { token_by_id, token_by_id_mut, tokens, MOIRToken, TokenId }
impl_get_symbol_by_id! { constructor_by_id, constructor_by_id_mut, constructors, MOIRConstructor, ConstructorId }
impl_get_symbol_by_id! { space_by_id, space_by_id_mut, spaces, MOIRSpace, SpaceId }

impl_get_symbol_id! { field_id_by_ident, FieldId, MOIRSymbolId::Field(id) => id}
impl_get_symbol_id! { custom_op_id_by_ident, CustomOpId, MOIRSymbolId::CustomOp(id) => id}
impl_get_symbol_id! { table_id_by_ident, TableId, MOIRSymbolId::Table(id) => id}
impl_get_symbol_id! { space_id_by_ident, SpaceId, MOIRSymbolId::Space(id) => id}
impl_get_symbol_id! { varnode_id_by_ident, VarNodeId, MOIRSymbolId::VarNode(id) => id}
