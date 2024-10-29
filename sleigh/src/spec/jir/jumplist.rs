use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Index;
use std::rc::Rc;

use chumsky::container::Container;
use itertools::{Either, Itertools};
use num::traits::Pow;
use num::PrimInt;

use crate::compiler::ast::pcode::deref;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::new_typed_index_table_id;
use crate::spec::moir::common::MOIRSize;
use crate::spec::typed_index_table::TypedIndexTable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NextJumplistEntry {
    Jumplist(JumplistId),
    Matcher(String),
}

#[derive(Debug, Clone)]
pub(crate) struct JumplistEntry {
    pub(crate) same_token_jumplists: Vec<JumplistId>,
    pub(crate) next_entry: NextJumplistEntry,
}

impl JumplistEntry {
    pub(crate) fn new(
        // all jumplists have to match successfully
        same_token_jumplists: Vec<JumplistId>,
        next_entry: NextJumplistEntry,
    ) -> Self {
        Self {
            same_token_jumplists,
            next_entry,
        }
    }
}

#[derive(Debug)]
pub(crate) enum JumplistContainer {
    // multiple alternatives are possible in case of a shared prefix
    Small([Option<Vec<JumplistEntry>>; 256]),
    Large(HashMap<u64, Vec<JumplistEntry>>),
}

#[derive(Debug)]
pub(crate) struct Jumplist {
    container: JumplistContainer,
    pub(crate) mask: u64, // usually only a few bits are actually relevant for indexing into the list
    pub(crate) bytes: MOIRSize,
}

// TODO put necessary information here
#[derive(Debug)]
pub(crate) struct JumplistMatching {
    tables: Vec<String>,
}

impl JumplistMatching {
    pub(crate) fn new() -> Self { Self { tables: Vec::new() } }

    pub fn merge(&mut self, mut other: JumplistMatching) {
        self.tables.append(&mut other.tables);
    }

    pub fn push(&mut self, table: String) {
        self.tables.push(table)
    }
}

impl Jumplist {
    pub(crate) fn new(
        mask: u64,
        bytes: MOIRSize,
    ) -> Self {
        let container = if bytes <= 1 {
            // JumplistContainer::Small([const { Option::None }; 256])
            JumplistContainer::Large(HashMap::new())
        } else {
            JumplistContainer::Large(HashMap::new())
        };

        Self { container, mask, bytes }
    }

    pub(crate) fn insert(
        &mut self,
        value: u64,
        entry: JumplistEntry,
    ) -> SleighCompilerResult {
        if value > 0 && value.ilog2() >= (self.bytes * 8) as u32 {
            todo!("Generate error report for out of bounds access");
        }

        match &mut self.container {
            JumplistContainer::Small(array) => {
                let array_entry = array
                    .get_mut(value as usize)
                    .expect("This has been checked before");
                if let Some(vec) = array_entry {
                    vec.push(entry);
                } else {
                    *array_entry = Some(vec![entry]);
                }
            }
            JumplistContainer::Large(map) => map.entry(value).or_insert(Vec::new()).push(entry),
        }

        Ok(())
    }

    // TODO correct return type
    pub(crate) fn r#match(&self, bytes: &[u8], jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>) -> SleighCompilerResult<Option<JumplistMatching>> {
        let value_bytes = bytes.get(0..(self.bytes as usize));
        if let Some(value_bytes) = value_bytes {
            let mut value_byte_arr = [0u8; 8];
            for (bytei, byte) in value_bytes.into_iter().enumerate() {
                value_byte_arr[bytei] = *byte;
            }

            let value = u64::from_le_bytes(value_byte_arr) & self.mask;
            if let Some(entries) = self.get(value)? {
                'next_entry: for entry in entries {
                    let mut matching = JumplistMatching::new();

                    // match all sublists
                    for sublist_id in &entry.same_token_jumplists {
                        let sublist = jumplist_cache.get(*sublist_id).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing jumplist") })?;
                        if let Some(list_matching) = sublist.r#match(bytes, jumplist_cache)? {
                            matching.merge(list_matching)
                        } else {
                            continue 'next_entry;
                        }
                    }

                    // match next entry while skipping already consumed bytes
                    match &entry.next_entry {
                        NextJumplistEntry::Jumplist(jlid) => {
                            let jumplist = jumplist_cache.get(*jlid).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing jumplist") })?;
                            if let Some(list_matching) = jumplist.r#match(&bytes[self.bytes as usize..], jumplist_cache)? {
                                matching.merge(list_matching);
                                return Ok(Some(matching));
                            }
                        }
                        NextJumplistEntry::Matcher(display) => {
                            matching.push(display.clone());
                            return Ok(Some(matching));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn get(
        &self,
        value: u64,
    ) -> SleighCompilerResult<Option<&Vec<JumplistEntry>>> {
        if value > 0 && value.ilog2() >= (self.bytes * 8) as u32 {
            todo!("Generate error report for out of bounds access");
        }

        Ok(match &self.container {
            JumplistContainer::Small(array) => array
                .get(value as usize)
                .expect("This has been checked before")
                .as_ref(),
            JumplistContainer::Large(map) => map.get(&value),
        })
    }
}

new_typed_index_table_id! { pub struct JumplistId; "JMPLST"; }
