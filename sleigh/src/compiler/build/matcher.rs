#![allow(unused_variables)]

use std::cmp::Ordering;
use std::collections::{
    HashMap,
    HashSet,
};

use itertools::Itertools;

use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::new_typed_index_table_id;
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::context::{
    MOIRContext,
    MOIRContextId,
};
use crate::spec::moir::endianness::MOIREndianness;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::moir::table::{
    MOIRTable,
    TableId,
};
use crate::spec::moir::token::{
    MOIRToken,
    TokenId,
};
use crate::spec::moir::varnode::VarNodeId;
use crate::spec::typed_index_table::TypedIndexTable;

// TODO: for now we assume each token to be of a maximum of 8 bytes, but this doesn't have to be
// the case probably

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldMatcherPattern {
    // r1=0xff00
    Value(u64),
    // r1=r2
    Equals(FieldId),
    // r1!=r2
    NotEquals(FieldId),
    // r1<r2
    LessThan(FieldId),
    // r1>r2
    GreaterThan(FieldId),
}

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub fn matches(
        &self,
        fields: &HashMap<FieldId, u64>,
    ) -> SleighCompilerResult<bool> {
        let field_value = *fields
            .get(&self.field)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing field value in field matcher");
            })?;
        match self.pattern {
            FieldMatcherPattern::Value(value) => Ok(field_value == value),
            FieldMatcherPattern::Equals(other) => {
                let other_value = *fields.get(&other).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing field value in field matcher");
                })?;
                Ok(field_value == other_value)
            }
            FieldMatcherPattern::NotEquals(other) => {
                let other_value = *fields.get(&other).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing field value in field matcher");
                })?;
                Ok(field_value != other_value)
            }
            FieldMatcherPattern::LessThan(other) => {
                let other_value = *fields.get(&other).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing field value in field matcher");
                })?;
                Ok(field_value < other_value)
            }
            FieldMatcherPattern::GreaterThan(other) => {
                let other_value = *fields.get(&other).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing field value in field matcher");
                })?;
                Ok(field_value > other_value)
            }
        }
    }
}

// every symbol type that can appear in the bit pattern section
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatcherAtom {
    Field(FieldMatcher),       // field=value or field=field
    Linkage(MatcherReference), // links a local symbol to a global symbol (subtable, varnode...)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenList {
    pub list: Vec<MOIRFieldParent>,
    pub justify_left: bool,
    pub justify_right: bool,
}

impl TokenList {
    pub fn new(list: Vec<MOIRFieldParent>, justify_left: bool, justify_right: bool) -> Self { Self { list, justify_left, justify_right } }
    pub fn remove_padding(&mut self) {
        self.justify_right = false;
        self.justify_left = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    // Each conjunction belongs to the corresponding MOIRFieldParent defined down below in
    // token_list.
    pub(super) conjunctions: Vec<HashSet<MatcherAtom>>,

    // ordered list of used tokens and context registers
    pub(super) token_list: TokenList,

    // symbols whose value needs to be extracted for later use (either for matching, displaying or semantics)
    pub(super) token_fields: Vec<HashSet<FieldId>>,

    // symbols whose presence needs to be known, but whose value does not matter or cannot be
    // extracted in the first place
    pub(super) references: HashSet<MatcherReference>,

    // TODO constructor will be linked here (no need for constructor -> matcher mapping)
    pub(super) constructor: (),
}

// ordering w.r.t. specificity
impl Ord for Matcher {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        if self.eq(other) {
            Ordering::Equal
        } else {
            if self.specificity() > other.specificity() {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

impl PartialOrd for Matcher {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        self.specificity().partial_cmp(&other.specificity())
    }
}

impl Matcher {
    pub fn new(
        conjunctions: Vec<HashSet<MatcherAtom>>,
        token_list: TokenList,
        token_fields: Vec<HashSet<FieldId>>,
        references: HashSet<MatcherReference>,
    ) -> Self {
        Self {
            conjunctions,
            token_list,
            token_fields,
            references,
            constructor: (),
        }
    }

    pub fn matches(
        &self,
        bytes: &[u8],
        endianness: MOIREndianness,
        // TODO find better way to pass all of this into here (create resolved matcher)
        moir_fields: &TypedIndexTable<FieldId, MOIRField>,
        moir_tokens: &TypedIndexTable<TokenId, MOIRToken>,
        moir_context: &TypedIndexTable<MOIRContextId, MOIRContext>,
        moir_tables: &TypedIndexTable<TableId, MOIRTable>,
    ) -> SleighCompilerResult<Option<Matching>> {
        let (fields, bytes_consumed) =
            self.extract_fields(bytes, endianness, moir_fields, moir_tokens, moir_context)?;

        for (conjunction, conjunction_fields) in self.conjunctions.iter().zip(fields.iter()) {
            for atom in conjunction {
                match atom {
                    MatcherAtom::Field(field_matcher) => {
                        if !field_matcher.matches(&conjunction_fields)? {
                            return Ok(None);
                        }
                    }
                    _ => {} // references don't affect the matching since everything is hoisted up
                            // already
                }
            }
        }

        // everything matches => call constructors recursively

        Ok(Some(Matching::new(fields, bytes_consumed)))
    }

    /// A matcher with a higher specificity needs to be checked before lower-specificity ones when
    /// iterating over all matchers to find a possible match.
    pub fn specificity(&self) -> usize {
        // TODO is this correct?
        self.token_fields.iter().map(HashSet::len).sum()
    }

    fn extract_fields(
        &self,
        bytes: &[u8],
        endianness: MOIREndianness,
        moir_fields: &TypedIndexTable<FieldId, MOIRField>,
        moir_tokens: &TypedIndexTable<TokenId, MOIRToken>,
        moir_context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    ) -> SleighCompilerResult<(Vec<HashMap<FieldId, u64>>, usize)> {
        // extract tokens (including context) from bytes
        let mut bytes_consumed: usize = 0;
        let mut token_values = Vec::new();
        for parent in self.token_list.list.iter() {
            match parent {
                MOIRFieldParent::Token(tid) => {
                    let moir_token = moir_tokens
                        .get(*tid)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing token");
                        })?;
                    let token_bytes = &bytes[bytes_consumed..((moir_token.bits / 8) as usize)];
                    let token_endian = if let Some(endian) = moir_token.endianness_override {
                        endian
                    } else {
                        endianness
                    };

                    let mut token_value = 0u64;

                    match token_endian {
                        MOIREndianness::Big => {
                            for (bytei, byte) in token_bytes.iter().enumerate().rev() {
                                token_value += (*byte as u64) << (8 * bytei);
                            }
                        }
                        MOIREndianness::Little => {
                            for (bytei, byte) in token_bytes.iter().enumerate() {
                                token_value += (*byte as u64) << (8 * bytei);
                            }
                        }
                    };

                    bytes_consumed += token_bytes.len();
                    token_values.push(token_value);
                }
                MOIRFieldParent::Context(cid) => {
                    let moir_context = moir_context
                        .get(*cid)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing context");
                        })?;
                    todo!("get varnode value");
                }
            }
        }

        // extract fields from tokens
        let fields = token_values
            .iter()
            .zip(self.token_fields.iter())
            .map(|(token_value, token_fields)| {
                token_fields
                    .iter()
                    .filter_map(|token_field| {
                        // TODO handle error
                        let moir_field = moir_fields
                            .get(*token_field)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing field");
                            })
                        .unwrap();
                        let field_value = moir_field.bits.extract_from(*token_value);
                        Some((*token_field, field_value))
                    })
                .collect::<HashMap<_, _>>()
            })
        .collect_vec();

        Ok((fields, bytes_consumed))
    }
}

new_typed_index_table_id! { pub struct MatcherId; "MTCHR"; }

// r0=0; r1=1...
