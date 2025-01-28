#![allow(unused_variables)]

use std::collections::{
    BTreeSet,
    HashMap,
    HashSet,
};

use crate::new_typed_index_table_id;
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::field::{
    FieldId,
    MOIRFieldParent,
};
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::VarNodeId;

// TODO: for now we assume each token to be of a maximum of 8 bytes, but this doesn't have to be
// the case probably

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug, Clone, PartialOrd, Ord, Copy, PartialEq, Eq, Hash)]
pub enum FieldMatcher {
    // matches everything (TODO can this be removed? might be needed somewhere)
    Wildcard,
    // everything else (except nontrivial expressions) is broken down to mask-value comparisons
    // TODO do we need to store the mask? do we still work with FieldIds later on or just bit
    // ranges? right now this is redundand
    Value { mask: u64, value: u64 },
}

// Every symbol type that can appear in the bit pattern section. This is only used for
// scope creation and verification and not for matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct TokenList {
    /// This can be `None` for shallow references when recursion is used
    pub list: Vec<Option<MOIRFieldParent>>,
    pub justify_left: bool,
    pub justify_right: bool,
}

impl TokenList {
    pub fn new(
        list: Vec<Option<MOIRFieldParent>>,
        justify_left: bool,
        justify_right: bool,
    ) -> Self {
        Self {
            list,
            justify_left,
            justify_right,
        }
    }
    pub fn remove_padding(&mut self) {
        self.justify_right = false;
        self.justify_left = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    // Each conjunction belongs to the corresponding MOIRFieldParent defined down below in
    // token_list.
    pub(crate) conjunctions: Vec<BTreeSet<MatcherAtom>>,

    // ordered list of used tokens and context registers
    pub(crate) token_list: TokenList,

    // Symbols whose presence needs to be known, but whose value does not matter or cannot be
    // extracted in the first place. Used for the scope.
    pub(crate) references: HashSet<MatcherReference>,

    pub(crate) table_id: TableId,
    pub(crate) constructor_id: ConstructorId,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Ord, PartialOrd)]
pub(crate) struct Submatchers {
    pub table: TableId,
    /// Matchers is Some:
    /// - Token list is concretized and can be used for verifying & constraints. Since not all
    /// matchers are compatible with each other, we don't just specify subtables but also a list of
    /// corresponding compatible matchers.

    /// Matchers is None:
    /// - Token list is not concretized; this is the case if recursion is encountered. Using it with
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

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq, Hash)]
pub(crate) enum MatcherAtom {
    Field { field_id: FieldId, matcher: FieldMatcher },
    Submatchers(Submatchers),
}

impl Matcher {
    pub fn new(
        conjunctions: Vec<BTreeSet<MatcherAtom>>,
        token_list: TokenList,
        references: HashSet<MatcherReference>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> Self {
        Self {
            conjunctions,
            token_list,
            references,
            table_id,
            constructor_id,
        }
    }
}

new_typed_index_table_id! { pub struct MatcherId; "MTCHR"; }
