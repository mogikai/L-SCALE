use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::compiler::ast::define::SASTEndianness;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use crate::compiler::CompilerContext;
use crate::spec::address_space::MOIRAddressSpace;
use crate::spec::moir::MOIRMacro;
use crate::spec::reporting::{
    generate_alignment_already_specified_report,
    generate_endianness_already_specified_report,
    generate_identifier_exists_report,
};
use crate::spec::symbol_table::MOIRSymbolTable;
use crate::spec::table::MOIRTable;
use crate::spec::varnode::MOIRVarNode;
use ariadne::Report;
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use string_interner::backend::{
    Backend,
    StringBackend,
};
use string_interner::{
    StringInterner,
    Symbol,
};
use thiserror::Error;
use tracing::debug;

pub mod address_space;
pub mod moir;
mod reporting;
mod symbol_table;
mod table;
pub mod varnode;

new_key_type! { pub struct MOIRIdent; }

#[derive(Debug, strum_macros::Display, Copy, Clone)]
pub enum ProcessorEndianness {
    Big,
    Little,
}

impl From<SASTEndianness> for ProcessorEndianness {
    fn from(endianness: SASTEndianness) -> Self {
        match endianness {
            SASTEndianness::Big => ProcessorEndianness::Big,
            SASTEndianness::Little => ProcessorEndianness::Little,
        }
    }
}

#[derive(Debug)]
pub enum MOIRSymbol {
    /// The identifier is not defined yet and can be used to define a symbol.
    Undefined,
    Macro(MOIRMacroId),
    Space(MOIRAddressSpaceId),
    Table(MOIRTableId),
}

#[derive(Debug, Default)]
pub struct MOIRMacroId(usize);
impl From<usize> for MOIRMacroId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
#[derive(Debug, Default, Copy, Clone)]
pub struct MOIRTableId(usize);
impl From<usize> for MOIRTableId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<MOIRTableId> for usize {
    fn from(value: MOIRTableId) -> Self {
        value.0
    }
}

#[derive(Debug, Default)]
pub struct MOIRAddressSpaceId(usize);
#[derive(Debug, Default)]
pub struct MOIRVarNodeId(usize);
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
/// StringId given out by interner. The interner is used by the compiler
/// to simplify string comparisons by mapping ids to strings.
pub struct StringId(usize);

#[derive(Debug)]
pub struct ProcessorSpecificationBuilder {
    interner: StringInterner<StringBackend>,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    symbols: SlotMap<MOIRIdent, MOIRSymbol>,
    // TODO: Remove public visibility once the api is more complete.
    /// Maps the identifier ids to corresponding strings.
    symbol_strings: SecondaryMap<MOIRIdent, Spanned<StringId>>,
    macros: MOIRSymbolTable<MOIRMacroId, MOIRMacro>,
    address_spaces: MOIRSymbolTable<MOIRAddressSpaceId, MOIRAddressSpace>,
    tables: MOIRSymbolTable<MOIRTableId, MOIRTable>,
    varnodes: MOIRSymbolTable<MOIRVarNodeId, MOIRVarNode>,
    /// The endianness of the processor. It must be defined.
    endianness: Option<Spanned<ProcessorEndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    alignment: Option<Spanned<u16>>,

    // lookup tables:
    string_to_ident_mapping: HashMap<StringId, MOIRIdent>,

    root_table: Spanned<StringId>,
}

impl ProcessorSpecificationBuilder {
    /// Define the endianness. If the endianness was previously defined, an error report is generated.
    pub(crate) fn define_endianness(
        &mut self,
        endianness: Spanned<ProcessorEndianness>,
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
    pub(crate) fn define_macro<'a>(
        &mut self,
        ident: Spanned<StringId>,
        moir_macro: MOIRMacro,
    ) -> Result<MOIRIdent, ProcessorSpecificationError> {
        let symbol_id = self.declare_symbol(ident)?;
        let macro_id = self.macros.insert(moir_macro);
        self.define_symbol(symbol_id, MOIRSymbol::Macro(macro_id))?;
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
            let table_id = self.tables.insert(MOIRTable::default());
            self.define_symbol(symbol_id, MOIRSymbol::Table(table_id))?;
            table_id
        };
        Ok(self
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
            macros: Default::default(),
            address_spaces: Default::default(),
            tables: Default::default(),
            varnodes: Default::default(),
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

#[derive(Debug, Default)]
pub struct ProcessorSpecification {}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Result<ProcessorSpecification, ProcessorSpecificationError> {
        debug!("Finalizing spec:");
        debug!("Found {} symbols", self.symbols.len());
        debug!("Found {} tables", self.tables.len());
        debug!("Found {} macros", self.macros.len());
        Ok(Default::default())
    }
}

#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The requested identifier is already used by something else.")]
    IdentAlreadyExists(InputSpan, InputSpan),
    #[error("The symbol must first be declared before it can be defined")]
    SymbolUndeclared,
    #[error("The processor specification builder produced a detailed error message:")]
    ErrorReport(Box<Report<'static, InputSpan>>),
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
