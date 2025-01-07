use std::collections::HashMap;

use chumsky::container::Container;
use itertools::Itertools;
use tracing::field;

use crate::compiler::SleighCompilerResult;
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::new_typed_index_table_id;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::trace_jir;
use crate::spec::moir::actions::MOIRActionTarget;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::MOIRContextId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
};
use crate::spec::moir::scope::MOIRLocalId;
use crate::spec::moir::varnode::MOIRReference;
use crate::spec::typed_index_table::TypedIndexTable;

use super::context_provider::ContextProvider;
use super::local_resolver::JIRResolver;
use super::values::JIRValue;

/// We need to do two passes for every constructor:
/// * get the constructor id and execute disassembly actions
/// * continue matching subtables
///
/// To avoid traversing the lists twice, we store necessary information here like
/// the remaining sublists with their byte slices.
#[derive(Debug)]
struct LocalMatching<'a> {
    pub sublists: Vec<&'a Vec<JumplistId>>,
    pub bytes: Vec<&'a [u8]>,
    pub constructor_id: ConstructorId,
}

impl<'a> LocalMatching<'a> {
    fn new(
        sublists: &'a Vec<JumplistId>,
        bytes: Vec<&'a [u8]>,
        constructor_id: ConstructorId,
    ) -> Self {
        Self {
            sublists,
            bytes,
            constructor_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NextJumplistEntry {
    // consume self.bytes bytes and continue with next
    TrailingConsume(JumplistId),
    // don't consume any bytes and continue with next
    TrailingSkip(JumplistId),
    Constructor(ConstructorId),
}

#[derive(Debug, Clone)]
pub(crate) struct JumplistEntry {
    pub(crate) sublists: Vec<JumplistId>,
    pub(crate) next_entry: NextJumplistEntry,
}

impl JumplistEntry {
    pub(crate) fn new(
        // all jumplists have to match successfully
        sublists: Vec<JumplistId>,
        next_entry: NextJumplistEntry,
    ) -> Self {
        Self { sublists, next_entry }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum JumplistContainer {
    // multiple alternatives are possible in case of a shared prefix
    Small([Option<Vec<JumplistEntry>>; 256]),
    Large(HashMap<u64, Vec<JumplistEntry>>),
}

impl Default for JumplistContainer {
    fn default() -> Self {
        JumplistContainer::Large(HashMap::default())
    }
}

// Data can come from either the byte stream, an external context provider.
// This needs to be accounted for when creating the lists.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, Hash, PartialEq, Eq, Default)]
pub(crate) enum DataProvider {
    #[default]
    ByteStream,
    Context(MOIRContextId),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Jumplist {
    container: JumplistContainer,
    pub(crate) mask: u64, // usually only a few bits are actually relevant for indexing into the list
    pub(crate) bytes: MOIRSize,
    pub(crate) data_provider: DataProvider,
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
        if value > 0 && (value.ilog2() + 1) > (self.bytes * 8) as u32 {
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
        context_provider: &ContextProvider,
        // TODO insert context providers
    ) -> SleighCompilerResult<Option<(u64, &'a [u8])>> {
        // TODO return value
        match self.data_provider {
            DataProvider::ByteStream => {
                let value_bytes = bytes.get(0..usize::from(self.bytes));
                match value_bytes {
                    Some(value_bytes) => {
                        let mut value_byte_arr = [0u8; 8];
                        for (bytei, byte) in value_bytes.into_iter().enumerate() {
                            value_byte_arr[bytei] = *byte;
                        }

                        let value = u64::from_le_bytes(value_byte_arr) & self.mask;
                        Ok(Some((value, &bytes[usize::from(self.bytes)..])))
                    }
                    None => Ok(None), // no bytes left
                }
            }
            DataProvider::Context(cid) => {
                let context_value = context.get(cid).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing context value");
                })?;
                Ok(Some((context_value & self.mask, bytes)))
            }
        }
    }

    /// Match the bit pattern and return the constructor id at the end while collecting tokens and
    /// sublists. This is necessary since we first need to extract fields and execute all actions
    /// before descending further.
    fn match_local(
        &self,
        bytes: &[u8],
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        context: &TypedIndexTable<MOIRContextId, u64>,
    ) -> SleighCompilerResult<Option<LocalMatching>> {
        let (value, remaining_bytes) = self
            .decode_value(bytes, context)?
            .expect("Couldn't decode value");
        eprintln!("bytes = {:#?}", bytes);
        eprintln!("value = {:#?}", value);

        if let Some(entries) = self.get(value)? {
            for entry in entries {
                eprintln!("entry = {:#?}", entry);

                // match next entry while skipping already consumed bytes
                match &entry.next_entry {
                    NextJumplistEntry::TrailingConsume(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing jumplist")
                            })?;

                        if let Some(mut lm) = jumplist.match_local(remaining_bytes, jumplist_cache, context)? {
                            lm.sublists.push(&entry.sublists);
                            lm.bytes.push(bytes);
                            return Ok(Some(lm));
                        }
                    }
                    NextJumplistEntry::TrailingSkip(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing jumplist")
                            })?;

                        if let Some(mut lm) = jumplist.match_local(bytes, jumplist_cache, context)? {
                            lm.sublists.push(&entry.sublists);
                            lm.bytes.push(bytes);
                            return Ok(Some(lm));
                        }
                    }
                    NextJumplistEntry::Constructor(cid) => {
                        return Ok(Some(LocalMatching::new(&entry.sublists, bytes, *cid)));
                    }
                };
            }
        }

        Ok(None)
    }

    // TODO correct return type
    pub(crate) fn r#match(
        &self,
        bytes: &[u8],
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        constructors: &TypedIndexTable<ConstructorId, MOIRConstructor>,
        fields: &TypedIndexTable<FieldId, MOIRField>,
        context: &mut TypedIndexTable<MOIRContextId, u64>,
    ) -> SleighCompilerResult<bool> {
        // TODO handle end of byte stream
        // let (value, remaining_bytes) = self.decode_value(bytes, context)?.expect("Couldn't decode value");

        // match next entry while skipping already consumed bytes
        let local_matching = match self.match_local(bytes, jumplist_cache, context)? {
            Some(lm) => lm,
            None => return Ok(false),
        };

        // entry matches with constructor => extract fields, execute actions, save values
        // and descent into sublists
        let constructor = constructors
            .get(local_matching.constructor_id)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing constructor");
            })?;
        trace_jir!("Got constructor {}", constructor.display);

        let mut field_values = constructor.local_scope.extract_fields(bytes);
        trace_jir!("Extracted field values {:#?}", field_values);

        let values = HashMap::with_capacity(context.len() + field_values.len());
        values.extend(field_values.into_iter());

        for action in &constructor.actions {
            let value = action.evaluate(values);
            match action.lhs {
                MOIRActionTarget::Context { id, bits } => {}
                MOIRActionTarget::Temporary(temp_id) => todo!("Handle temporary values"),
            }
        }

        // byte=0xF3; mod=2 & Rmr64; row=1 [opsize=2; addrsize=row+2] {  }
        // byte=0xF3; mod=3 & Rmr64; row=2 [opsize=1; addrsize=row+1] {  }

        Ok(true)

        // match all sublists with the modified context
        // let mut sublists_match = true;
        // for sublist_id in &entry.same_token_jumplists {
        //     let sublist = jumplist_cache
        //         .get(*sublist_id)
        //         .ok_or_else(|| -> SleighCompilerError {
        //             todo!("Generate error report for missing jumplist")
        //         })?;
        //     let sublist_resolver = JIRResolver::new(spec, constructor, inst_start, inst_next, inst_next2)
        //         if let Some(submatching) = sublist.r#match(bytes, jumplist_cache)? {
        //             // TODO nothing
        //             matching.merge(submatching);
        //         } else {
        //             sublists_match = false;
        //             break;
        //         }
        // }

        // if !sublists_match {
        //     continue;
        // }
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

    pub(crate) fn entries<'a>(&'a self) -> Box<dyn Iterator<Item = (u64, &'a Vec<JumplistEntry>)> + 'a> {
        match &self.container {
            JumplistContainer::Small(array) => Box::new(array.iter().enumerate().filter_map(|(value, option)| {
                option
                    .as_ref()
                    .map(|entries| (u64::try_from(value).unwrap(), entries))
            })),
            JumplistContainer::Large(map) => Box::new(map.iter().map(|(value, entries)| (*value, entries))),
        }
    }
}

new_typed_index_table_id! { pub struct JumplistId; "JMPLST"; }
