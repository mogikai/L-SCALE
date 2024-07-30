use slotmap::{
    new_key_type,
    SecondaryMap,
    SlotMap,
};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{
    Display,
    Formatter,
    Pointer,
};
use std::ops::Index;
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

new_key_type! { pub struct MOIRAddressSpaceId; }
new_key_type! { pub struct MOIRVarNodeId; }
new_key_type! { pub struct MOIRIdentId; }

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
pub struct MOIRTable;
pub struct MOIRTableId(usize);

#[derive(Debug, Default)]
pub struct ProcessorSpecificationBuilder {
    pub(super) interner: StringInterner<StringBackend>,

    // TODO: we probably need a symbol table here that maps identifiers to symbols.
    pub(super) identifiers: SlotMap<MOIRIdentId, Spanned<StringId>>,
    pub(super) macros: SecondaryMap<MOIRIdentId, MOIRMacro>,
    pub(super) address_spaces: SlotMap<MOIRAddressSpaceId, MOIRAddressSpace>,
    pub(super) tables: Vec<MOIRTable>,
    pub(super) varnodes: SlotMap<MOIRVarNodeId, MOIRVarNode>,
    /// The endianness of the processor. It must be defined.
    pub(super) endianness: Option<Spanned<ProcessorEndianness>>,
    /// The alignment of instructions. This defaults to 1 if no alignment is specified.
    pub(super) alignment: Option<Spanned<u16>>,

    // lookup tables:
    pub(super) string_to_ident_mapping: HashMap<StringId, MOIRIdentId>,
}

impl ProcessorSpecificationBuilder {
    /// Resolve the given StringId to the corresponding identifier id.
    pub(crate) fn resolve_ident_to_id<I>(
        &mut self,
        ident: I,
    ) -> Option<MOIRIdentId>
    where
        I: Into<StringId>,
    {
        self.string_to_ident_mapping.get(&ident.into()).copied()
    }

    pub(crate) fn reserve_global_ident(
        &mut self,
        (ident, span): Spanned<StringId>,
    ) -> Result<MOIRIdentId, ProcessorSpecificationError> {
        match self.string_to_ident_mapping.entry(ident) {
            Entry::Occupied(entry) => Err(ProcessorSpecificationError::IdentAlreadyExists(
                self.identifiers.get(*entry.get()).unwrap().1.clone(),
                span,
            )),
            Entry::Vacant(entry) => Ok(*entry.insert(self.identifiers.insert((ident, span)))),
        }
    }

    pub(crate) fn resolve_ident_to_str(
        &self,
        ident: StringId,
    ) -> &str {
        self.string(ident)
            .expect("The string must have been interned if we gave out an id")
    }
}

impl ProcessorSpecificationBuilder {
    pub fn new() -> Self {
        Self {
            interner: Default::default(),
            identifiers: Default::default(),
            macros: Default::default(),
            address_spaces: Default::default(),
            tables: Default::default(),
            varnodes: Default::default(),
            endianness: Default::default(),
            alignment: Default::default(),
            string_to_ident_mapping: Default::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessorSpecificationError {
    #[error("The requested identifier is already used by something else.")]
    IdentAlreadyExists(InputSpan, InputSpan),
}
