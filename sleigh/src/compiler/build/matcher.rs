#![allow(unused_variables)]
use slotmap::new_key_type;

use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::field::{FieldId, MOIRFieldParent};
use crate::spec::moir::table::TableId;
use crate::spec::moir::token::TokenId;
use crate::spec::moir::varnode::VarNodeId;

// TODO: for now we assume each token to be of a maximum of 8 bytes, but this doesn't have to be
// the case probably

#[derive(Debug, Clone)]
pub enum FieldMatcherPattern {
    // r1=0xff00
    BitPattern {
        value: u64,
        shift: u8,
        mask: u64,
    },
    // r1=r2
    Field(FieldId),
}

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug, Clone)]
pub struct FieldMatcher {
    pub(super) field: FieldId,
    pub(super) pattern: FieldMatcherPattern,
}

impl FieldMatcher {
    pub fn new(
        field: FieldId,
        pattern: FieldMatcherPattern,
    ) -> Self {
        Self { field, pattern }
    }
}

// the left-hand side of a matcher expression can be a reference to either a field or a subtable
// TODO do we miss symbols?
#[derive(Debug, Clone)]
pub enum MatcherReference {
    Field(FieldId),
    Subtable(TableId),
    Varnode(VarNodeId),
}

#[derive(Debug, Clone)]
pub enum MatcherAtom {
    Field(FieldMatcher), // field=value or field=field
    Linkage(MatcherReference),  // links a local symbol to a global symbol (subtable, varnode...)
}

/// Or and Concat expressions are removed so we end up with only And at the end
#[derive(Debug, Clone)]
pub struct MatcherConjunction {
    pub(super) matcher_atoms: Vec<MatcherAtom>,
}

impl MatcherConjunction {
    pub fn new(matcher_atoms: Vec<MatcherAtom>) -> Self {
        Self {
            matcher_atoms,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatcherToken {
    Determined(MOIRFieldParent),
    Undetermined, // put at either end of list for left/right justify
}

/// Match a token against an expression using field matchers.
#[derive(Debug)]
pub struct Matcher {
    // Each conjunction belongs to the corresponding MOIRFieldParent defined down below in
    // token_list.
    pub(super) conjunctions: Vec<MatcherConjunction>,
    // ordered list of used tokens and context registers
    pub(super) token_list: Vec<MatcherToken>,

    // this is known before the matcher is run on anything and should only
    // contain fields as well as MatcherReference types (i.e. fields, subtables and varnodes)
    pub(super) symbols_used: Vec<MOIRSymbolId>,
}

/// A list of all symbols that matched are implicitly given through the matcher
/// TODO find good structure
#[derive(Debug)]
pub enum Matching {
    FieldConstant(FieldId, u64),
    SubtableConstant,
    SubtableVarNode,
}

impl Matcher {
    pub fn new(
        conjunctions: Vec<MatcherConjunction>,
        // we need to include fields from context as well to be able to extract them later, but
        // they are skipped when validating e.g. token equality for And expressions
        token_list: Vec<MatcherToken>,
        symbols_used: Vec<MOIRSymbolId>,
    ) -> Self {
        Self {
            conjunctions,
            token_list,
            symbols_used,
        }
    }

    pub fn r#match(
        &self,
        bytes: &[u8],
    ) -> Option<Vec<Matching>> {
        todo!("Match")
    }
}

new_key_type! { pub struct MatcherId; }
