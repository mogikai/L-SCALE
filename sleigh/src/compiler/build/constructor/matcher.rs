//! Normal form for matchers.
//!
//! Matching expressions can become arbitrarily complex, so we represent them in a standard form as
//! a list of [MatcherConjunction]s, each of which consisting of a token and associated
//! constraints.

#![allow(unused_variables)]

use crate::new_typed_index_table_id;
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::field::{
    FieldId,
    MOIRFieldParent,
};
use crate::spec::moir::field_extractor::FieldExtractor;
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::VarNodeId;
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::collections::{
    BTreeSet,
    HashMap,
    HashSet,
};

// TODO: for now we assume each token to be of a maximum of 8 bytes, but this doesn't have to be
// the case probably

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug, Clone, PartialOrd, Ord, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldMatcher {
    // matches everything (TODO can this be removed? might be needed somewhere)
    Wildcard,
    // everything else (except nontrivial expressions) is broken down to mask-value comparisons
    // TODO do we need to store the mask? do we still work with FieldIds later on or just bit
    // ranges? right now this is redundand
    Value { mask: u128, value: u128 },
}

// Every symbol type that can appear in the bit pattern section. This is only used for
// scope creation and verification and not for matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatcherReference {
    Field(FieldId),
    Subtable(TableId),
    Varnode(VarNodeId),
}

impl From<MatcherReference> for MOIRSymbolId {
    fn from(value: MatcherReference) -> Self {
        match value {
            MatcherReference::Field(fid) => MOIRSymbolId::Field(fid),
            MatcherReference::Subtable(stid) => MOIRSymbolId::Table(stid),
            MatcherReference::Varnode(vnid) => MOIRSymbolId::VarNode(vnid),
        }
    }
}

impl From<&MatcherReference> for MOIRSymbolId {
    fn from(value: &MatcherReference) -> Self {
        match value {
            MatcherReference::Field(fid) => MOIRSymbolId::Field(*fid),
            MatcherReference::Subtable(stid) => MOIRSymbolId::Table(*stid),
            MatcherReference::Varnode(vnid) => MOIRSymbolId::VarNode(*vnid),
        }
    }
}

#[derive(Debug)]
pub struct Matching {
    pub fields: Vec<HashMap<FieldId, u64>>,
    pub bytes_consumed: usize,
}

impl Matching {
    pub fn new(
        fields: Vec<HashMap<FieldId, u64>>,
        bytes_consumed: usize,
    ) -> Self {
        Self { fields, bytes_consumed }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatcherToken {
    /// This can be `None` for subtables
    pub parent: Option<MOIRFieldParent>,
}

impl MatcherToken {
    pub fn new(parent: Option<MOIRFieldParent>) -> Self {
        Self { parent }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatcherConjunction {
    // atoms are always assigned with a token, either context or regular
    pub(crate) atoms: BTreeSet<MatcherAtom>,
    pub(crate) token: MatcherToken,

    // dynamic-offset field extractors and submatchers are stored per conjunction and created
    // in the constructor with their corresponding locals
    pub(crate) dynamic_submatchers: Vec<Submatchers>,
    pub(crate) dynamic_field_extractors: Vec<FieldExtractor>,
}

impl Default for MatcherConjunction {
    fn default() -> Self {
        Self {
            atoms: Default::default(),
            token: MatcherToken::new(None),
            dynamic_submatchers: Default::default(),
            dynamic_field_extractors: Default::default(),
        }
    }
}

// :ADD is (byte=0x3F; r64) | (byte_subtable; r64) { ... }

impl MatcherConjunction {
    pub fn new(
        atoms: BTreeSet<MatcherAtom>,
        token: MatcherToken,
    ) -> Self {
        Self {
            atoms,
            token,
            dynamic_submatchers: Default::default(),
            dynamic_field_extractors: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) struct OffsettedSubmatchers {
    pub(crate) offset: usize,
    pub(crate) submatchers: Submatchers,
}

impl OffsettedSubmatchers {
    pub(crate) fn new(
        offset: usize,
        submatchers: Submatchers,
    ) -> Self {
        Self { offset, submatchers }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matcher {
    // Each conjunction belongs to the corresponding MOIRFieldParent defined down below in
    // token_list.
    pub(crate) conjunctions: Vec<MatcherConjunction>,

    // Symbols whose presence needs to be known, but whose value does not matter or cannot be
    // extracted in the first place. Used for the scope.
    pub(crate) references: HashSet<MatcherReference>,

    pub(crate) table_id: TableId,
    pub(crate) constructor_id: ConstructorId,

    // fixed-offset field extractors and submatchers are stored per matcher and created
    // in the constructor with their corresponding locals
    pub(crate) static_submatchers: Vec<OffsettedSubmatchers>,
    pub(crate) static_field_extractors: Vec<FieldExtractor>,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Ord, PartialOrd, Serialize, Deserialize)]
pub(crate) struct Submatchers {
    pub table: TableId,
    /// Matchers is Some:
    /// - Token list is concretized and can be used for verifying & constraints. Since not all
    /// matchers are compatible with each other, we don't just specify subtables but also a list of
    /// corresponding compatible matchers.

    /// Matchers is None:
    /// - Token list is not concretized; this is the case for subtables. Using it with
    /// & constraints will constrain it.
    pub matchers: Option<Vec<MatcherId>>,
}

impl Submatchers {
    pub(crate) fn new(
        table: TableId,
        matchers: Option<Vec<MatcherId>>,
    ) -> Self {
        Self { table, matchers }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub(crate) enum MatcherAtom {
    Field { field_id: FieldId, matcher: FieldMatcher },
    Submatchers(Submatchers),
}

impl Matcher {
    pub(crate) fn new(
        conjunctions: Vec<MatcherConjunction>,
        references: HashSet<MatcherReference>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> Self {
        Self {
            conjunctions,
            references,
            table_id,
            constructor_id,
            static_submatchers: Default::default(),
            static_field_extractors: Default::default(),
        }
    }
}

new_typed_index_table_id! { pub struct MatcherId; "MTCHR"; }
