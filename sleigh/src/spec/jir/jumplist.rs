use super::context_stack::ContextStack;
use super::local_resolver::JIRResolver;
use crate::compiler::InternalCompilerResult;
use crate::compiler::error::InternalError;
use crate::new_typed_index_table_id;
use crate::spec::jir::trace_jir;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::ContextId;
use crate::spec::moir::table::TableId;
use crate::spec::typed_index_table::TypedIndexTable;
use fnv::FnvHashMap;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use serde_big_array::BigArray;
use serde_derive::{
    Deserialize,
    Serialize,
};

/// We need to do two passes for every constructor:
/// * get the constructor id and execute disassembly actions
/// * continue matching subtables
///
/// To avoid traversing the lists twice, we store necessary information here like
/// the remaining sublists with their byte slices.
#[derive(Debug, Default)]
struct LocalMatching<'a> {
    sublists: Vec<&'a Vec<JumplistId>>,
    bytes: Vec<&'a [u8]>,
    tokens: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum NextJumplistEntry {
    // consume self.bytes bytes and continue with next
    TrailingNext(JumplistId),
    // don't consume any bytes and continue with next
    TrailingSame(JumplistId),
    Constructor { cid: ConstructorId, tid: TableId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum JumplistContainer {
    // multiple alternatives are possible in case of a shared prefix
    #[serde(with = "BigArray")]
    Small([Option<Vec<JumplistEntry>>; 256]),
    Large(FnvHashMap<u64, Vec<JumplistEntry>>),
}

impl Default for JumplistContainer {
    fn default() -> Self {
        JumplistContainer::Large(Default::default())
    }
}

// Data can come from either the byte stream, an external context provider.
// This needs to be accounted for when creating the lists.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, Hash, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) enum DataProvider {
    #[default]
    ByteStream,
    Context(ContextId),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Jumplist {
    container: JumplistContainer,
    mask: u64, // usually only a few bits are actually relevant for indexing into the list
    bytes: MOIRSize,
    data_provider: DataProvider,
}

impl Jumplist {
    pub(crate) fn new(
        mask: u64,
        bytes: MOIRSize,
        data_provider: DataProvider,
    ) -> Self {
        let container = if bytes <= 1 {
            JumplistContainer::Small([const { None }; 256])
        } else {
            JumplistContainer::Large(Default::default())
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
    ) -> InternalCompilerResult {
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
        context_provider: &ContextStack,
    ) -> InternalCompilerResult<Option<(u64, &'a [u8])>> {
        // TODO return value
        match self.data_provider {
            DataProvider::ByteStream => {
                let mut value_bytes = [0u8; 8];
                for (bytei, byte) in bytes.iter().enumerate().take(self.bytes.into()) {
                    value_bytes[bytei] = *byte;
                }
                let value = u64::from_le_bytes(value_bytes);
                Ok(Some((value, &bytes[usize::from(self.bytes)..])))
            }
            DataProvider::Context(cid) => {
                let context_value = context_provider.context_value(cid)?;
                Ok(Some((context_value, bytes)))
            }
        }
    }

    /// Match the bit pattern and return the constructor id at the end while collecting tokens and
    /// sublists. This is necessary since we first need to extract fields and execute all actions
    /// before descending further. Since there might be multiple local matchings,
    /// we return a list ordered by priority with the most specific ones first.
    fn match_local<'a, 'b>(
        &'b self,
        jumplist_cache: &'b TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_provider: &mut ContextStack,
        bytes: &'b [u8],
        accum: &mut LocalMatching<'b>,
    ) -> InternalCompilerResult<Option<(TableId, JIRResolver<'a>)>> {
        // field extraction ignores the mask so we need the unmasked value as well
        let (value_unmasked, remaining_bytes) = self
            .decode_value(bytes, context_provider)?
            .expect("Couldn't decode value");

        let value_masked = value_unmasked & self.mask;

        if let Some(entries) = self.get(value_masked)? {
            for entry in entries {
                // store state for backtracking
                accum.sublists.push(&entry.sublists);
                accum.bytes.push(bytes);

                let result = match &entry.next_entry {
                    NextJumplistEntry::Constructor { cid, tid } => {
                        trace_jir!("Reached leaf; cid={cid:?}, tid={tid:?}");

                        accum.tokens.push(value_unmasked);
                        let result = self.process_local_matching(
                            jumplist_cache,
                            constructors,
                            context_provider,
                            accum,
                            *cid,
                            *tid,
                        )?;

                        result
                    }
                    NextJumplistEntry::TrailingNext(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        // store token only if we skip and consume something
                        accum.tokens.push(value_unmasked);
                        let result = jumplist.match_local(
                            jumplist_cache,
                            constructors,
                            context_provider,
                            remaining_bytes,
                            accum,
                        )?;
                        if result.is_none() {
                            // backtrack
                            accum.tokens.pop();
                        }

                        result
                    }
                    NextJumplistEntry::TrailingSame(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        jumplist.match_local(jumplist_cache, constructors, context_provider, bytes, accum)?
                    }
                };

                if let Some(result) = result {
                    // successful termination (including subtables) => end
                    return Ok(Some(result));
                }

                // no complete match => backtrack and try next entry
                accum.sublists.pop();
                accum.bytes.pop();
            }
        }

        // no match
        Ok(None)
    }

    fn process_local_matching<'a, 'b>(
        &'b self,
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_provider: &mut ContextStack,
        local_matching: &LocalMatching<'b>,
        constructor_id: ConstructorId,
        table_id: TableId,
    ) -> InternalCompilerResult<Option<(TableId, JIRResolver<'a>)>> {
        // entry matches with constructor => extract fields, execute actions, save values
        // and descent into sublists

        let constructor = constructors
            .get(constructor_id)
            .ok_or_else(|| -> InternalError {
                todo!("Generate error report for missing constructor");
            })?;

        trace_jir!(
            "Got constructor {} with {} actions and {} field extractors",
            constructor.display,
            constructor.actions.len(),
            constructor.field_extractors.len()
        );

        // extract token and context fields
        let mut values = SecondaryMap::default();

        for field_extractor in &constructor.field_extractors {
            field_extractor.process_value(&local_matching.tokens, &mut values)?;
        }

        // the resolver with all local values is needed for the actions so we
        // instantiate it here
        let mut resolver = JIRResolver::new(
            constructor,
            values,
            EVAddr::from(0xDEADBEEFu64),
            EVAddr::from(0xDEADBEEFu64),
            Some(EVAddr::from(0xDEADBEEFu64)),
        );

        for (context_field_id, context_field_value) in context_provider.values() {
            if resolver.has_field(context_field_id) {
                trace_jir!("Defining context field value {context_field_id:?} := {context_field_value:?}");
                resolver.define_field(context_field_id, context_field_value)?;
            }
        }

        context_provider.push_state();

        for action in &constructor.actions {
            action.evaluate(&mut resolver, context_provider)?;
        }

        let mut sublist_resolvers = Vec::new();
        for (sublist_ids, sublist_bytes) in local_matching
            .sublists
            .iter()
            .zip(local_matching.bytes.iter())
        {
            for sublist_id in *sublist_ids {
                let sublist = jumplist_cache
                    .get(*sublist_id)
                    .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;
                trace_jir!(
                    "Descending into sublist {:#?} with bytes={:#x?} (from {})",
                    sublist_id,
                    *sublist_bytes,
                    constructor.display
                );
                let (table_id, resolver) =
                    match sublist.r#match(*sublist_bytes, jumplist_cache, constructors, context_provider)? {
                        Some(resolver) => resolver,
                        None => {
                            // no match => try next
                            trace_jir!("No match, trying next (if existing)");
                            context_provider.pop_state()?;
                            return Ok(None);
                        }
                    };
                sublist_resolvers.push((table_id, resolver));

                trace_jir!("Sublist {sublist_id} matches");
            }
        }

        trace_jir!("Got a complete match");

        // everything matched

        // TODO handle context flow
        context_provider.pop_state()?;

        // finish resolver
        for (table_id, sublist_resolver) in sublist_resolvers {
            // TODO somehow get the table id to here
            trace_jir!("Adding subtable resolver {table_id:?}");
            resolver.define_subtable_resolver(table_id, sublist_resolver)?;
        }

        return Ok(Some((table_id, resolver)));
    }

    // TODO correct return type
    pub(crate) fn r#match<'a, 'b>(
        &'b self,
        bytes: &'b [u8],
        jumplist_cache: &'b TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_provider: &mut ContextStack,
    ) -> InternalCompilerResult<Option<(TableId, JIRResolver<'a>)>> {
        let mut accum = LocalMatching::default();
        self.match_local(jumplist_cache, constructors, context_provider, bytes, &mut accum)
    }

    pub(crate) fn get(
        &self,
        value: u64,
    ) -> InternalCompilerResult<Option<&Vec<JumplistEntry>>> {
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
