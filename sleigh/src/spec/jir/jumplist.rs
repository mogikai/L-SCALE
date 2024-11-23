use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Index;
use std::rc::Rc;

use chumsky::container::Container;
use itertools::{
    Either,
    Itertools,
};
use num::PrimInt;
use num::traits::Pow;

use crate::compiler::SleighCompilerResult;
use crate::compiler::ast::pcode::deref;
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::new_typed_index_table_id;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::context::MOIRContextId;
use crate::spec::typed_index_table::TypedIndexTable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NextJumplistEntry {
    Jumplist(JumplistId),
    Matcher(String),
    // we need a stub in case of entries that only link to another list without
    // consuming and without any next entries (e.g. for submasks)
    Stub,
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

// Data can come from either the byte stream or an external context provider.
// This needs to be accounted for when creating the lists.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, Hash, PartialEq, Eq)]
pub(crate) enum DataProvider {
    ByteStream,
    Context(MOIRContextId),
}

#[derive(Debug)]
pub(crate) struct Jumplist {
    container: JumplistContainer,
    pub(crate) mask: u64, // usually only a few bits are actually relevant for indexing into the list
    pub(crate) bytes: MOIRSize,
    pub(crate) data_provider: DataProvider,
}

// TODO put necessary information here
#[derive(Debug)]
pub(crate) struct JumplistMatching {
    tables: Vec<String>,
}

impl JumplistMatching {
    pub(crate) fn new() -> Self {
        Self { tables: Vec::new() }
    }

    pub fn merge(
        &mut self,
        mut other: JumplistMatching,
    ) {
        self.tables.append(&mut other.tables);
    }

    pub fn push(
        &mut self,
        table: String,
    ) {
        self.tables.push(table)
    }
}

impl Jumplist {
    pub(crate) fn new(
        mask: u64,
        bytes: MOIRSize,
        data_provider: DataProvider,
    ) -> Self {
        let container = if bytes <= 1 {
            // JumplistContainer::Small([const { Option::None }; 256])
            JumplistContainer::Large(HashMap::new())
        } else {
            JumplistContainer::Large(HashMap::new())
        };

        Self {
            container,
            mask,
            bytes,
            data_provider,
        }
    }

    pub(crate) fn insert(
        &mut self,
        value: u64,
        entry: JumplistEntry,
        prepend: bool,
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
                    if prepend {
                        vec.insert(0, entry);
                    } else {
                        vec.push(entry);
                    }
                } else {
                    *array_entry = Some(vec![entry]);
                }
            }
            JumplistContainer::Large(map) => {
                let vec = map.entry(value).or_insert(Vec::new());
                if prepend {
                    vec.insert(0, entry);
                } else {
                    vec.push(entry);
                }
            }
        }

        Ok(())
    }

    fn decode_value<'a>(
        &'a self,
        bytes: &'a [u8],
        // TODO insert context providers
    ) -> Option<(u64, &'a [u8])> {
        // TODO return value
        match self.data_provider {
            DataProvider::ByteStream => {
                let value_bytes = bytes.get(0..usize::try_from(self.bytes).unwrap());
                match value_bytes {
                    Some(value_bytes) => {
                        let mut value_byte_arr = [0u8; 8];
                        for (bytei, byte) in value_bytes.into_iter().enumerate() {
                            value_byte_arr[bytei] = *byte;
                        }

                        let value = u64::from_le_bytes(value_byte_arr) & self.mask;
                        Some((value, &bytes[usize::try_from(self.bytes).unwrap()..]))
                    }
                    None => None, // no bytes left
                }
            }
            DataProvider::Context(cid) => {
                // todo!("Provide context values")
                Some((0x20010c021 & self.mask, bytes))
            }
        }
    }

    // TODO correct return type
    pub(crate) fn r#match(
        &self,
        bytes: &[u8],
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        // TODO insert context providers
    ) -> SleighCompilerResult<Option<JumplistMatching>> {
        // TODO handle
        let (value, remaining_bytes) = self.decode_value(bytes).expect("Couldn't decode value");
        trace_build!("DESCENDING");

        if let Some(entries) = self.get(value)? {
            for entry in entries {
                let mut matching = JumplistMatching::new();

                // match all sublists
                let mut sublists_match = true;
                for sublist_id in &entry.same_token_jumplists {
                    let sublist = jumplist_cache
                        .get(*sublist_id)
                        .ok_or_else(|| -> SleighCompilerError {
                            todo!("Generate error report for missing jumplist")
                        })?;
                    if let Some(list_matching) = sublist.r#match(bytes, jumplist_cache)? {
                        matching.merge(list_matching)
                    } else {
                        sublists_match = false;
                        break;
                    }
                }

                if !sublists_match {
                    continue;
                }

                // match next entry while skipping already consumed bytes
                match &entry.next_entry {
                    NextJumplistEntry::Jumplist(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing jumplist")
                            })?;
                        if let Some(list_matching) = jumplist.r#match(remaining_bytes, jumplist_cache)? {
                            matching.merge(list_matching);
                            return Ok(Some(matching));
                        }
                    }
                    NextJumplistEntry::Matcher(display) => {
                        matching.push(display.clone());
                        return Ok(Some(matching));
                    }
                    NextJumplistEntry::Stub => return Ok(Some(matching)),
                }
            }
        }

        // not enough bytes left
        Ok(None)
    }

    pub(crate) fn get(
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

    pub(crate) fn values<'a>(&'a self) -> Box<dyn Iterator<Item = u64> + 'a> {
        match &self.container {
            JumplistContainer::Small(array) => Box::new(
                (0..array.len())
                    .filter(|index| array.get(*index).expect("We checked for this").is_some())
                    .map(|index| u64::try_from(index).unwrap()),
            ),
            JumplistContainer::Large(map) => Box::new(map.keys().copied()),
        }
    }

    pub(crate) fn entries_mut<'a>(&'a mut self) -> Box<dyn Iterator<Item = (u64, &'a mut Vec<JumplistEntry>)> + 'a> {
        match &mut self.container {
            JumplistContainer::Small(array) => Box::new(array.iter_mut().enumerate().filter_map(|(value, option)| {
                option
                    .as_mut()
                    .map(|entries| (u64::try_from(value).unwrap(), entries))
            })),
            JumplistContainer::Large(map) => Box::new(map.iter_mut().map(|(value, entries)| (*value, entries))),
        }
    }
}

new_typed_index_table_id! { pub struct JumplistId; "JMPLST"; }
