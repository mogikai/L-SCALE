use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Index;
use std::rc::Rc;

use chumsky::container::Container;
use itertools::Itertools;
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
    pub(crate) same_token_jumplist: Vec<JumplistId>,
    pub(crate) next_entry: NextJumplistEntry,
}

impl JumplistEntry {
    pub(crate) fn new(
        // all jumplists have to match successfully
        same_token_jumplist: Vec<JumplistId>,
        next_entry: NextJumplistEntry,
    ) -> Self {
        Self {
            same_token_jumplist,
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
    mask: u64, // usually only a few bits are actually relevant for indexing into the list
    bytes: MOIRSize,
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
        eprintln!("value = {:#?}", value);
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

    pub(crate) fn for_entries_mut<F>(&mut self, f: &mut F)
        where F: FnMut(&mut JumplistEntry)
        {
            match &mut self.container {
                JumplistContainer::Small(array) => for entries in array.iter_mut().filter_map(|entry| entry.as_mut()) {
                    for entry in entries.iter_mut() {
                        f(entry);
                    }
                }
                JumplistContainer::Large(map) => for entries in map.values_mut() {
                    for entry in entries.iter_mut() {
                        f(entry);
                    }
                }
            }
        }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (u64, &Vec<JumplistEntry>)> {
        // TODO do this without collecting into vector
        match &self.container {
            JumplistContainer::Small(array) => array
                .iter()
                .enumerate()
                .filter_map(|(value, entry)| match entry {
                    Some(entry) => Some((value as u64, entry)),
                    None => None,
                })
            .collect_vec()
                .into_iter(),
            JumplistContainer::Large(map) => map
                .iter()
                .map(|(value, entry)| (*value, entry))
                .collect_vec()
                .into_iter(),
        }
    }

    pub(crate) fn mask(&self) -> u64 {
        self.mask
    }

    pub(crate) fn bytes(&self) -> MOIRSize {
        self.bytes
    }
}

#[derive(Debug)]
pub(crate) struct JIRTable {
    jumplists: TypedIndexTable<JumplistId, Jumplist>, // multiple jumplists for different token lengths
}

impl JIRTable {
    pub(crate) fn new(jumplists: TypedIndexTable<JumplistId, Jumplist>) -> Self {
        Self { jumplists }
    }
}

new_typed_index_table_id! { pub struct JumplistId; "JMPLST"; }
