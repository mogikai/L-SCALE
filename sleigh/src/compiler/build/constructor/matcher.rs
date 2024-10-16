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
use crate::spec::moir::constructor::ConstructorId;
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

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldMatcher {
    // matches everything (TODO can this be removed? might be needed somewhere)
    Wildcard,
    // everything else (except nontrivial expressions) is broken down to mask-value comparisons
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
    pub list: Vec<MOIRFieldParent>,
    pub justify_left: bool,
    pub justify_right: bool,
}

impl TokenList {
    pub fn new(
        list: Vec<MOIRFieldParent>,
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
    pub(crate) conjunctions: Vec<HashSet<MatcherAtom>>,

    // ordered list of used tokens and context registers
    pub(crate) token_list: TokenList,

    // symbols whose presence needs to be known, but whose value does not matter or cannot be
    // extracted in the first place
    pub(crate) references: HashSet<MatcherReference>,

    pub(crate) constructor: Option<ConstructorId>,
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
    Field { field: FieldId, matcher: FieldMatcher },
    Submatchers { table: TableId, matchers: Vec<MatcherId> },
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
            constructor: None, // this will be set later
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

        if let Some(tokens) = tokens {
            let mut field_values: Vec<HashMap<FieldId, u64>> = vec![HashMap::new(); self.conjunctions.len()];

            if self.matches_with_tokens(&tokens[..], &mut field_values[..], moir_fields, moir_tables)? {
                return Ok(Some(Matching::new(field_values, bytes_consumed)));
            }
        }

        Ok(None)
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
    ) -> SleighCompilerResult<(Option<Vec<u64>>, usize)> {
        // extract tokens (including context) from bytes
        let mut bytes_consumed: usize = 0;
        let mut token_values = Vec::with_capacity(self.token_list.list.len());
        for parent in self.token_list.list.iter() {
            match parent {
                MOIRFieldParent::Token(tid) => {
                    let moir_token = moir_tokens
                        .get(*tid)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing token");
                        })?;
                    // TODO handle end of stream (no bytes remaining)
                    let token_bytes = &bytes.get(bytes_consumed..(bytes_consumed + (moir_token.bytes) as usize));
                    match token_bytes {
                        Some(bytes) => {
                            let token_endian = if let Some(endian) = moir_token.endianness_override {
                                endian
                            } else {
                                endianness
                            };

                            let mut token_value = 0u64;

                            match token_endian {
                                MOIREndianness::Big => {
                                    for (bytei, byte) in bytes.iter().enumerate().rev() {
                                        token_value += (*byte as u64) << (8 * bytei);
                                    }
                                }
                                MOIREndianness::Little => {
                                    for (bytei, byte) in bytes.iter().enumerate() {
                                        token_value += (*byte as u64) << (8 * bytei);
                                    }
                                }
                            };

                            bytes_consumed += bytes.len();
                            token_values.push(token_value);
                        }
                        None => return Ok((None, bytes_consumed)),
                    }
                }
                MOIRFieldParent::Context(cid) => {
                    let moir_context = moir_context
                        .get(*cid)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing context");
                        })?;
                    // let varnode_value =
                    //     varnode_values
                    //         .get(&moir_context.varnode)
                    //         .ok_or_else(|| -> SleighCompilerError {
                    //             todo!("Generate error report for missing varnode value");
                    //         })?;
                    let varnode_value = 0;
                    token_values.push(varnode_value);
                }
            }
        }

        Ok((Some(token_values), bytes_consumed))
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
                    MatcherAtom::Field { field, matcher } => {
                        if let FieldMatcher::Value { mask, value } = matcher {
                            let token_value = extracted_tokens
                                .get(i)
                                .expect("Not enough tokens left for matching");
                            if token_value & mask != *value {
                                return Ok(false);
                            }
                        }
                    }
                    MatcherAtom::Submatchers { table, matchers } => {
                        let subtable = moir_tables
                            .get(*table)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing table");
                            })?;
                        let mut has_match = false;
                        for matcher_id in matchers {
                            let matcher =
                                subtable
                                    .matchers
                                    .get(*matcher_id)
                                    .ok_or_else(|| -> SleighCompilerError {
                                        todo!("Generate error report for missing submatcher");
                                    })?;
                            if matcher.matches_with_tokens(
                                &extracted_tokens[i..],
                                &mut field_values[i..],
                                moir_fields,
                                moir_tables,
                            )? {
                                has_match = true;
                                break;
                            }
                        }

                        if !has_match {
                            return Ok(false);
                        }
                    }
                }
            }
        }

        Ok(true)
    }
}

new_typed_index_table_id! { pub struct MatcherId; "MTCHR"; }

// r0=0; r1=1...
