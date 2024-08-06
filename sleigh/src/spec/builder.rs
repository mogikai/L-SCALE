//! The [ProcessorSpecificationBuilder] is used by the compiler to build up the [ProcessorSpecification].
//! This is done, so that the compiler can work on an incomplete specification and do some final validations,
//! before building the final [ProcessorSpecification].
//! The final [ProcessorSpecification] cannot be modified further.
//! TODO: Improve docs

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use slotmap::{
    SecondaryMap,
    SlotMap,
};
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::symbol::Symbol;
use string_interner::StringInterner;
use tracing::debug;

use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use crate::compiler::CompilerContext;
use crate::spec::address_space::MOIRAddressSpace;
use crate::spec::endianness::MOIREndianness;
use crate::spec::field::{
    MOIRField,
    MOIRFieldId,
};
use crate::spec::moir::MOIRMacro;
use crate::spec::reporting::{
    generate_alignment_already_specified_report,
    generate_endianness_already_specified_report,
    generate_identifier_exists_report,
};
use crate::spec::symbol_table::MOIRSymbolTable;
use crate::spec::table::MOIRTable;
use crate::spec::token::{
    MOIRToken,
    MOIRTokenId,
};
use crate::spec::varnode::MOIRVarNode;
use crate::spec::{
    MOIRAddressSpaceId,
    MOIRIdent,
    MOIRMacroId,
    MOIRSymbol,
    MOIRTableId,
    MOIRVarNodeId,
    ProcessorSpecification,
    ProcessorSpecificationError,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
/// StringId given out by interner. The interner is used by the compiler
/// to simplify string comparisons by mapping ids to strings.
pub struct StringId(usize);

#[derive(Debug, Default)]
/// A transparent helper struct that contains all the symbol tables to make the builder more manageable.
struct SymbolTables {
    macros: MOIRSymbolTable<MOIRMacroId, MOIRMacro>,
    address_spaces: MOIRSymbolTable<MOIRAddressSpaceId, MOIRAddressSpace>,
    tables: MOIRSymbolTable<MOIRTableId, MOIRTable>,
    varnodes: MOIRSymbolTable<MOIRVarNodeId, MOIRVarNode>,
    tokens: MOIRSymbolTable<MOIRTokenId, MOIRToken>,
    fields: MOIRSymbolTable<MOIRFieldId, MOIRField>,
}

#[derive(Debug)]
pub struct ProcessorSpecificationBuilder {
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
    /// The endianness of the processor. It must be defined.
    endianness: Option<Spanned<MOIREndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    alignment: Option<Spanned<u16>>,

    /// Maps a string identifier to the corresponding internal [MOIRIdent].
    /// This is useful for the compiler, when an identifier string references another symbol.
    /// This will need to be resolved to the correct [MOIRIdent], so that the PSB can then
    /// use the internal identifiers, instead of the strings.
    string_to_ident_mapping: HashMap<StringId, MOIRIdent>,

    root_table: Spanned<StringId>,
}

impl ProcessorSpecificationBuilder {
    /// Define the endianness. If the endianness was previously defined, an error report is generated.
    pub(crate) fn define_endianness(
        &mut self,
        endianness: Spanned<MOIREndianness>,
    ) -> Result<(), ProcessorSpecificationError> {
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
    ) -> Result<(), ProcessorSpecificationError> {
        if let Some(old_alignment) = &self.alignment {
            Err(generate_alignment_already_specified_report(old_alignment, alignment.into_span()))
        } else {
            self.alignment = Some(alignment.transmute());
            Ok(())
        }
    }

    /// Define a new macro identified by `ident`.
    /// If `ident` was already defined previously,
    /// an error report is returned, describing
    /// where the definition occurred.
    pub(crate) fn define_macro(
        &mut self,
        ident: Spanned<StringId>,
        moir_macro: MOIRMacro,
    ) -> Result<MOIRIdent, ProcessorSpecificationError> {
        let symbol_id = self.declare_symbol(ident)?;
        let macro_id = self.symbol_tables.macros.insert(moir_macro);
        self.define_symbol(symbol_id, MOIRSymbol::Macro(macro_id))?;
        Ok(symbol_id)
    }

    pub(crate) fn define_token(
        &mut self,
        ident: Spanned<StringId>,
        token: MOIRToken,
    ) -> Result<MOIRTokenId, ProcessorSpecificationError> {
        let symbol_id = self.declare_symbol(ident)?;
        let token_id = self.symbol_tables.tokens.insert(token);
        self.define_symbol(symbol_id, MOIRSymbol::Token(token_id))?;
        Ok(token_id)
    }

    pub(crate) fn define_field(
        &mut self,
        ident: Spanned<StringId>,
        field: MOIRField,
    ) -> Result<MOIRIdent, ProcessorSpecificationError> {
        let symbol_id = self.declare_symbol(ident)?;
        let field_id = self.symbol_tables.fields.insert(field);
        self.define_symbol(symbol_id, MOIRSymbol::Field(field_id))?;
        Ok(symbol_id)
    }

    pub(crate) fn modify_root_table(&mut self) -> Result<&mut MOIRTable, ProcessorSpecificationError> {
        self.modify_table(self.root_table.clone())
    }

    pub(crate) fn modify_table(
        &mut self,
        ident: Spanned<StringId>,
    ) -> Result<&mut MOIRTable, ProcessorSpecificationError> {
        let table_id = if let Some((symbol, _)) = self.get_symbol_and_id(&ident) {
            // Symbol is defined. Do stuff with it.
            if let MOIRSymbol::Table(table_id) = symbol {
                *table_id
            } else {
                todo!("Generate error report")
            }
        } else {
            // Symbol needs to be defined first.
            let symbol_id = self.declare_symbol(ident)?;
            let table_id = self.symbol_tables.tables.insert(MOIRTable::default());
            self.define_symbol(symbol_id, MOIRSymbol::Table(table_id))?;
            table_id
        };
        Ok(self
            .symbol_tables
            .tables
            .get_mut(table_id)
            .expect("We either just inserted the new table, or fetched an existing one."))
    }

    fn get_symbol_and_id(
        &self,
        ident: &Spanned<StringId>,
    ) -> Option<(&MOIRSymbol, MOIRIdent)> {
        self.string_to_ident_mapping.get(ident).map(|ident| {
            (
                self.symbols
                    .get(*ident)
                    .expect("If the ident is mapped, then there has to exist a symbol."),
                *ident,
            )
        })
    }

    /// Attempts to declare a symbol with the provided identifier.
    /// If the identifier is already occupied, an error is returned.
    /// Otherwise, a reference to the newly allocated symbol slot and its id is returned.
    fn declare_symbol(
        &mut self,
        ident: Spanned<StringId>,
    ) -> Result<MOIRIdent, ProcessorSpecificationError> {
        match self.string_to_ident_mapping.entry(*ident) {
            // The identifier is already declared, and we do not allow re-declaring a symbol.
            Entry::Occupied(entry) => {
                let name = self
                    .symbol_strings
                    .get(*entry.get())
                    .expect("The symbol string must exist");
                let name_str = self.resolve_string_id(**name);
                Err(generate_identifier_exists_report(name_str, name.span().clone(), ident.into_span()))
            }
            Entry::Vacant(entry) => {
                // No symbol has been declared yet. We can now insert an empty declaration.
                let id = self.symbols.insert(MOIRSymbol::Undefined);
                self.symbol_strings.insert(id, ident);
                entry.insert(id);
                Ok(id)
            }
        }
    }

    /// Helper function to define a symbol after declaring it.
    fn define_symbol(
        &mut self,
        symbol_id: MOIRIdent,
        symbol: MOIRSymbol,
    ) -> Result<(), ProcessorSpecificationError> {
        *self
            .symbols
            .get_mut(symbol_id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared)? = symbol;
        Ok(())
    }

    pub(crate) fn resolve_string_id(
        &self,
        ident: StringId,
    ) -> &str {
        self.string(ident)
            .expect("The string must have been interned if we gave out an id")
    }
}

impl Default for ProcessorSpecificationBuilder {
    fn default() -> Self {
        let mut interner = StringInterner::new();
        let root_table = Spanned::new(interner.intern(&"instruction"), InputSpan::default());
        let mut slf = Self {
            interner,
            symbols: Default::default(),
            symbol_strings: Default::default(),
            symbol_tables: Default::default(),
            endianness: None,
            alignment: None,
            string_to_ident_mapping: Default::default(),
            root_table,
        };
        // Call modify once to create the table.
        slf.modify_root_table()
            .expect("This may not error at this point");
        slf
    }
}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Result<ProcessorSpecification, ProcessorSpecificationError> {
        debug!("Finalizing spec:");
        debug!("Found {} symbols", self.symbols.len());
        debug!("Found {} tables", self.symbol_tables.tables.len());
        debug!("Found {} macros", self.symbol_tables.macros.len());
        debug!("Found {} tokens", self.symbol_tables.tokens.len());
        debug!("Found {} fields", self.symbol_tables.fields.len());
        Ok(Default::default())
    }
}

impl CompilerContext for StringInterner<StringBackend> {
    fn intern<T: AsRef<str>>(
        &mut self,
        string: &T,
    ) -> StringId {
        StringId(self.get_or_intern(string).to_usize())
    }

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
        string: &T,
    ) -> StringId {
        self.interner.intern(string)
    }

    /// Resolve a string id to the corresponding interned string.
    fn string(
        &self,
        string_id: StringId,
    ) -> Option<&str> {
        self.interner.string(string_id)
    }
}
