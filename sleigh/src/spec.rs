use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{
    Display,
    Pointer,
};
use std::marker::PhantomData;
use std::ops::Index;

use ariadne::{
    ColorGenerator,
    Fmt,
    Label,
    Report,
    ReportKind,
};
use chumsky::span::Span;
use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use string_interner::backend::StringBackend;
use string_interner::StringInterner;
use thiserror::Error;

use crate::compiler::ast::define::SASTEndianness;
use crate::compiler::ast::Spanned;
use crate::compiler::lexer::InputSpan;
use crate::compiler::{
    CompilerContext,
    StringId,
};
use crate::spec::address_space::MOIRAddressSpace;
use crate::spec::moir::MOIRMacro;
use crate::spec::varnode::MOIRVarNode;

pub mod address_space;
pub mod moir;
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

#[derive(Debug, Default)]
pub struct MOIRTable;

#[derive(Debug)]
pub enum MOIRSymbol {
    /// The identifier is not defined yet and can be used to define a symbol.
    Undefined,
    Macro(MOIRMacroId),
    Space(MOIRAddressSpaceId),
}

#[derive(Debug, Default)]
struct MOIRSymbolTable<K, V>(Vec<V>, PhantomData<K>);

impl<K, V> MOIRSymbolTable<K, V>
where
    K: From<usize>,
{
    pub fn insert(
        &mut self,
        element: V,
    ) -> K {
        self.0.push(element);
        K::from(self.0.len() - 1)
    }
}

#[derive(Debug, Default)]
pub struct MOIRMacroId(usize);
impl From<usize> for MOIRMacroId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
#[derive(Debug, Default)]
pub struct MOIRTableId(usize);
#[derive(Debug, Default)]
pub struct MOIRAddressSpaceId(usize);
#[derive(Debug, Default)]
pub struct MOIRVarNodeId(usize);

#[derive(Debug, Default)]
pub struct ProcessorSpecificationBuilder {
    pub(super) interner: StringInterner<StringBackend>,

    /// The main symbol map of the specification.
    /// Each identifier points to a specific symbol id.
    /// The symbols are saved in separate lists, one for each symbol type.
    symbols: SlotMap<MOIRIdent, MOIRSymbol>,
    // TODO: Remove public visibility once the api is more complete.
    /// Maps the identifier ids to corresponding strings.
    pub(super) symbol_strings: SecondaryMap<MOIRIdent, Spanned<StringId>>,
    macros: MOIRSymbolTable<MOIRMacroId, MOIRMacro>,
    pub(super) address_spaces: MOIRSymbolTable<MOIRAddressSpaceId, MOIRAddressSpace>,
    pub(super) tables: MOIRSymbolTable<MOIRTableId, MOIRTable>,
    pub(super) varnodes: MOIRSymbolTable<MOIRVarNodeId, MOIRVarNode>,
    /// The endianness of the processor. It must be defined.
    pub(super) endianness: Option<Spanned<ProcessorEndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    pub(super) alignment: Option<Spanned<u16>>,

    // lookup tables:
    string_to_ident_mapping: HashMap<StringId, MOIRIdent>,
}

impl ProcessorSpecificationBuilder {
    /// Define a new macro identified by `ident`.
    /// If `ident` was already defined previously,
    /// an error report is returned, describing
    /// where the definition occurred.
    pub(crate) fn define_macro<'a>(
        &mut self,
        ident: Spanned<StringId>,
        moir_macro: MOIRMacro,
    ) -> Result<MOIRIdent, Report<'a, InputSpan>> {
        self.define_symbol_with(ident, move |slf| MOIRSymbol::Macro(slf.macros.insert(moir_macro)))
    }

    /// Attempts to declare a symbol with the provided identifier.
    /// If the identifier is already occupied, an error is returned.
    /// Otherwise, a reference to the newly allocated symbol slot and its id is returned.
    fn define_symbol_with<'a, F>(
        &mut self,
        (ident, ident_span): Spanned<StringId>,
        definition: F,
    ) -> Result<MOIRIdent, Report<'a, InputSpan>>
    where
        F: FnOnce(&mut Self) -> MOIRSymbol,
    {
        match self.string_to_ident_mapping.entry(ident) {
            // The identifier is already declared, and we do not allow re-declaring a symbol.
            Entry::Occupied(entry) => {
                let (name, span) = self
                    .symbol_strings
                    .get(*entry.get())
                    .expect("The symbol string must exist");
                let name = self.resolve_string_id(*name);
                Err(generate_identifier_exists_report(name, span.clone(), ident_span))
            }
            Entry::Vacant(entry) => {
                // No symbol has been declared yet. We can now insert an empty declaration.
                let id = self.symbols.insert(MOIRSymbol::Undefined);
                entry.insert(id);
                *self.symbols.get_mut(id).expect("We just inserted this") = definition(self);
                Ok(id)
            }
        }
    }

    pub(crate) fn resolve_string_id(
        &self,
        ident: StringId,
    ) -> &str {
        self.string(ident)
            .expect("The string must have been interned if we gave out an id")
    }
}

/// Generates an error report for IdentAlreadyExists errors.
fn generate_identifier_exists_report<'a>(
    name: &str,
    old: InputSpan,
    new: InputSpan,
) -> Report<'a, InputSpan> {
    let mut colors = ColorGenerator::new();
    let color_a = colors.next();
    let report = Report::build(ReportKind::Error, new.context(), new.start())
        .with_message("Identifier already defined")
        .with_label(
            Label::new(new)
                .with_message(format!("Identifier {} is already defined", name.fg(color_a)))
                .with_color(color_a),
        )
        .with_label(
            Label::new(old)
                .with_message(format!("Identifier {} previously defined here", name.fg(color_a)))
                .with_color(color_a),
        )
        .finish();
    report
}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The requested identifier is already used by something else.")]
    IdentAlreadyExists(InputSpan, InputSpan),
}
