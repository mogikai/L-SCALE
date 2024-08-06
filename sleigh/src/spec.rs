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
pub mod builder;
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
