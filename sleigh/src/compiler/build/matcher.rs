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
    // matches everything
    Wildcard,
    // r1=0xff00
    Value(u64),
    // TODO do we need to add more general expressions? can we match fields in different tokens?
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

    pub fn secondary_field(&self) -> Option<FieldId> {
        match self.pattern {
            FieldMatcherPattern::Wildcard => None,
            FieldMatcherPattern::Value(_) => None,
            FieldMatcherPattern::Equals(fid) => Some(fid),
            FieldMatcherPattern::NotEquals(fid) => Some(fid),
            FieldMatcherPattern::LessThan(fid) => Some(fid),
            FieldMatcherPattern::GreaterThan(fid) => Some(fid),
        }
    }

    pub fn matches(
        &self,
        primary: u64,
        secondary: Option<u64>,
    ) -> SleighCompilerResult<bool> {
        match self.pattern {
            FieldMatcherPattern::Wildcard => Ok(true),
            FieldMatcherPattern::Value(value) => Ok(primary == value),
            FieldMatcherPattern::Equals(_) => {
                secondary.ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing secondary field") }).map(|v| {
                    primary == v
                })
            }
            FieldMatcherPattern::NotEquals(_) => {
                secondary.ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing secondary field") }).map(|v| {
                    primary != v
                })
            }
            FieldMatcherPattern::LessThan(_) => {
                secondary.ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing secondary field") }).map(|v| {
                    primary < v
                })
            }
            FieldMatcherPattern::GreaterThan(_) => {
                secondary.ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing secondary field") }).map(|v| {
                    primary > v
                })
            }
        }
    }
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum MatcherAtom {
    Field(FieldMatcher),
    Submatchers {
        table: TableId,
        matchers: Vec<MatcherId>,
    },
}

impl Matcher {
    pub fn new(
        conjunctions: Vec<HashSet<MatcherAtom>>,
        token_list: TokenList,
        references: HashSet<MatcherReference>,
    ) -> Self {
        Self {
            conjunctions,
            token_list,
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
        varnode_values: &HashMap<VarNodeId, u64>, // for context tokens
        moir_tables: &TypedIndexTable<TableId, MOIRTable>,
    ) -> SleighCompilerResult<Option<Matching>> {
        let (tokens, bytes_consumed) =
            self.extract_tokens(bytes, endianness, moir_tokens, moir_context, varnode_values)?;

        // TODO the scope should determine necessary fields and the matcher builder validates
        // this, not the other way around due to ambiguities/duplicate fields
        let mut field_values: Vec<HashMap<FieldId, u64>> = vec![HashMap::new(); self.conjunctions.len()];

        if self.matches_with_tokens(&tokens[..], &mut field_values[..], moir_fields, moir_tables)? {
            Ok(Some(Matching::new(field_values, bytes_consumed)))
        } else {
            Ok(None)
        }
    }

    /// A matcher with a higher specificity needs to be checked before lower-specificity ones when
    /// iterating over all matchers to find a possible match.
    pub fn specificity(&self) -> usize {
        // TODO find correct metric
        self.conjunctions.iter().map(HashSet::len).sum()
    }

    fn extract_tokens(
        &self,
        bytes: &[u8],
        endianness: MOIREndianness,
        moir_tokens: &TypedIndexTable<TokenId, MOIRToken>,
        moir_context: &TypedIndexTable<MOIRContextId, MOIRContext>,
        varnode_values: &HashMap<VarNodeId, u64>, // for context tokens
    ) -> SleighCompilerResult<(Vec<u64>, usize)> {
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
                    // TODO handle end of stream (no bytes remaining)
                    let token_bytes = &bytes[bytes_consumed..(bytes_consumed + (moir_token.bits / 8) as usize)];
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
                    let varnode_value = varnode_values.get(&moir_context.varnode)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing varnode value");
                        })?;
                    token_values.push(*varnode_value);
                }
            }
        }

        Ok((token_values, bytes_consumed))
    }

    /// Matches itself using a slice of already extracted token values.
    ///
    /// # Return value
    /// - Some(fields) if there was a match
    /// - None else
    fn matches_with_tokens(
        &self,
        extracted_tokens: &[u64],
        field_values: &mut [HashMap<FieldId, u64>],
        moir_fields: &TypedIndexTable<FieldId, MOIRField>,
        moir_tables: &TypedIndexTable<TableId, MOIRTable>,
    ) -> SleighCompilerResult<bool> {
        for (i, conjunction) in self.conjunctions.iter().enumerate() {
            for atom in conjunction {
                match atom {
                    MatcherAtom::Field(field_matcher) => {
                        let fields = field_values.get_mut(i).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing field map"); })?;
                        // extract fields and run field matcher
                        let primary = moir_fields.get(field_matcher.field).map(|field| {
                            field.bits.extract_from(*extracted_tokens.get(i).expect("There should always be enough tokens"))
                        }).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing field"); })?;
                        fields.insert(field_matcher.field, primary);
                        let secondary = match field_matcher.secondary_field() {
                            Some(field_id) => {
                                let value = moir_fields.get(field_id).map(|field| {
                                    field.bits.extract_from(*extracted_tokens.get(i).expect("There should always be enough tokens"))
                                }).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing field"); })?;
                                fields.insert(field_id, value);
                                Some(value)
                            },
                            None => None,
                        };

                        if !field_matcher.matches(primary, secondary)? {
                            return Ok(false);
                        }
                    },
                    MatcherAtom::Submatchers { table, matchers } => {
                        let subtable = moir_tables.get(*table).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing table"); })?;
                        for matcher_id in matchers {
                            let matcher = subtable.matchers.get(*matcher_id).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing submatcher"); })?;
                            if matcher.matches_with_tokens(&extracted_tokens[i..], &mut field_values[i..], moir_fields, moir_tables)? {
                                return Ok(true);
                            }
                        }

                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }
}

new_typed_index_table_id! { pub struct MatcherId; "MTCHR"; }

// r0=0; r1=1...
