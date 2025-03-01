use super::context_stack::ContextStack;
use super::local_resolver::JIRResolver;
use super::values::JIRValue;
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
use crate::spec::moir::field_extractor::FieldExtractor;
use crate::spec::moir::scope::MOIRLocalId;
use crate::spec::moir::table::TableId;
use crate::spec::typed_index_table::TypedIndexTable;
use fnv::FnvHashMap;
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::SecondaryMap;
use serde_derive::{
    Deserialize,
    Serialize,
};

/// We need to do two passes for every constructor:
/// * get the constructor id and execute disassembly actions
/// * continue matching subtables
///
/// To avoid traversing the lists twice, we store necessary information here like
/// the remaining sublists.
#[derive(Debug, Default)]
struct LocalMatching<'a> {
    // collect sublist resolvers (no sublists since we immediately descend) and values with a
    // dynamic offset and store as vec for backtracking
    subresolvers: Vec<(TableId, JIRResolver<'a>)>,
    values: Vec<(MOIRLocalId, Option<JIRValue>)>,
}

#[derive(Debug)]
pub(crate) enum MatcherResult<'a> {
    // TODO should this result in an early return or in a continuation of matching?
    EndOfByteStream,
    None,
    Some {
        table: TableId,
        resolver: JIRResolver<'a>,
        bytes_consumed: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct OffsettedJumplist {
    offset: usize,
    jumplist: JumplistId,
}

impl OffsettedJumplist {
    pub(crate) fn new(
        offset: usize,
        jumplist: JumplistId,
    ) -> Self {
        Self { offset, jumplist }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum NextJumplistEntry {
    // consume self.bytes bytes and continue with next
    TrailingNext(JumplistId),
    // don't consume any bytes and continue with next
    TrailingSame(JumplistId),
    Constructor {
        cid: ConstructorId,
        tid: TableId,
        // fixed-offset sublists and extractors
        sublists: Vec<OffsettedJumplist>,
        field_extractors: Vec<FieldExtractor>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JumplistEntry {
    pub(crate) next_entry: NextJumplistEntry,
    // not all field positions are known beforehand so we store additional extractors and lists here
    pub(crate) sublists: Vec<JumplistId>,
    pub(crate) field_extractors: Vec<FieldExtractor>,
}

impl JumplistEntry {
    pub(crate) fn new(
        // all jumplists have to match successfully
        next_entry: NextJumplistEntry,
    ) -> Self {
        Self {
            next_entry,
            sublists: Default::default(),
            field_extractors: Default::default(),
        }
    }

    pub(crate) fn with_sublists(
        // all jumplists have to match successfully
        next_entry: NextJumplistEntry,
        sublists: Vec<JumplistId>,
    ) -> Self {
        Self {
            next_entry,
            sublists,
            field_extractors: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum JumplistContainer {
    // multiple alternatives are possible in case of a shared prefix
    Small(Vec<Option<Vec<JumplistEntry>>>),
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
        let container = if bytes == 1 {
            JumplistContainer::Small(vec![None; 256])
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

    fn decode_value(
        &self,
        bytes: &[u8],
        consumed_so_far: usize,
        context_stack: &ContextStack,
    ) -> InternalCompilerResult<Option<(u64, usize)>> {
        match self.data_provider {
            DataProvider::ByteStream => {
                if bytes.len() - consumed_so_far < self.bytes.into() {
                    // no bytes left
                    return Ok(None);
                }

                let offsetted_bytes = &bytes[consumed_so_far..];

                let mut value_bytes = [0u8; 8];
                for (bytei, byte) in offsetted_bytes.iter().enumerate().take(self.bytes.into()) {
                    value_bytes[bytei] = *byte;
                }
                let value = u64::from_le_bytes(value_bytes);
                Ok(Some((value & self.mask, usize::from(self.bytes))))
            }
            DataProvider::Context(cid) => {
                let context_value = context_stack.context_value(cid)?;
                Ok(Some((context_value & self.mask, 0)))
            }
        }
    }

    /// Match the bit pattern while descending into partially constrained sublists and continuing
    /// with trailing bytes and tokens.
    fn match_local<'a, 'b>(
        &'b self,
        bytes: &[u8],
        addr: EVAddr,
        jumplist_cache: &'b TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
        consumed_so_far: usize,
        accum: &mut LocalMatching<'a>,
    ) -> InternalCompilerResult<MatcherResult<'a>> {
        let (value_masked, mut consumed) = self
            .decode_value(bytes, consumed_so_far, context_stack)?
            .expect("Couldn't decode value");

        if let Some(entries) = self.get(value_masked)? {
            'next_entry: for entry in entries {
                // store state for backtracking
                let (prev_len_subresolvers, prev_len_values) = (accum.subresolvers.len(), accum.values.len());

                // immediately descend into any variable-offset sublists and add resolver if match
                for sublist_id in &entry.sublists {
                    trace_jir!("Descending into entry sublist {} with bytes_consumed={}", sublist_id, consumed_so_far);

                    let sublist = jumplist_cache
                        .get(*sublist_id)
                        .expect("Invalid jumplist id");

                    match sublist.r#match(
                        &bytes[consumed_so_far..],
                        addr,
                        jumplist_cache,
                        constructors,
                        context_stack,
                    )? {
                        MatcherResult::EndOfByteStream => return Ok(MatcherResult::EndOfByteStream), // TODO should we continue?
                        MatcherResult::None => continue 'next_entry,
                        MatcherResult::Some {
                            table,
                            resolver,
                            bytes_consumed,
                        } => {
                            // if we have dynamic sublists, they dictate how much is consumed
                            consumed = consumed.max(bytes_consumed);
                            accum.subresolvers.push((table, resolver));
                        }
                    }
                }

                // variable-offset extractors
                for field_extractor in &entry.field_extractors {
                    trace_jir!(
                        "Extracting local {} with entry extractor {:#?}",
                        field_extractor.local_id(),
                        field_extractor
                    );
                    let extracted_value = field_extractor.extract_value(bytes)?;
                    accum
                        .values
                        .push((field_extractor.local_id(), extracted_value));
                }

                let result = match &entry.next_entry {
                    NextJumplistEntry::Constructor {
                        cid,
                        tid,
                        sublists,
                        field_extractors,
                    } => {
                        trace_jir!("Reached leaf; cid={cid:?}, tid={tid:?}");

                        self.process_local_matching(
                            bytes,
                            addr,
                            jumplist_cache,
                            constructors,
                            context_stack,
                            consumed_so_far + consumed,
                            accum,
                            sublists,
                            field_extractors,
                            *cid,
                            *tid,
                        )?
                    }
                    NextJumplistEntry::TrailingNext(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        jumplist.match_local(
                            bytes,
                            addr,
                            jumplist_cache,
                            constructors,
                            context_stack,
                            consumed_so_far + consumed,
                            accum,
                        )?
                    }
                    NextJumplistEntry::TrailingSame(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        jumplist.match_local(
                            bytes,
                            addr,
                            jumplist_cache,
                            constructors,
                            context_stack,
                            consumed_so_far,
                            accum,
                        )?
                    }
                };

                match result {
                    MatcherResult::None => {
                        // no complete match => backtrack and try next entry
                        accum.subresolvers.truncate(prev_len_subresolvers);
                        accum.values.truncate(prev_len_values);
                    }
                    // complete match or end of stream => return
                    _ => return Ok(result),
                }
            }
        }

        // no match
        Ok(MatcherResult::None)
    }

    fn process_local_matching<'a, 'b>(
        &'b self,
        bytes: &'b [u8],
        addr: EVAddr,
        jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
        consumed_so_far: usize,
        local_matching: &LocalMatching<'a>,
        // fixed offset
        sublists: &Vec<OffsettedJumplist>,
        field_extractors: &Vec<FieldExtractor>,
        constructor_id: ConstructorId,
        table_id: TableId,
    ) -> InternalCompilerResult<MatcherResult<'a>> {
        // entry matches with constructor => extract fields, execute actions, save values
        // and descent into sublists

        let constructor = constructors
            .get(constructor_id)
            .ok_or_else(|| -> InternalError {
                todo!("Generate error report for missing constructor");
            })?;

        trace_jir!(
            "Got constructor {} with {} actions, consumed {} bytes",
            constructor.display,
            constructor.actions.len(),
            consumed_so_far,
        );

        trace_jir!("Sublists: {:#?}", sublists);
        trace_jir!("Field extractors: {:#?}", field_extractors);

        let mut values = SecondaryMap::<MOIRLocalId, Option<JIRValue>>::default();

        // traversing values from front to back overwrites old values
        for &(local_id, value) in local_matching.values.iter() {
            values[local_id] = value;
        }

        // extract token and context fields with fixed offset into map
        for field_extractor in field_extractors {
            let extracted_value = field_extractor.extract_value(bytes)?;
            values[field_extractor.local_id()] = extracted_value;
        }

        // the resolver with all local values is needed for the actions so we
        // instantiate it here
        let mut resolver = JIRResolver::new(constructor, values);

        for (subtable, subresolver) in &local_matching.subresolvers {
            resolver.define_subtable_resolver(*subtable, subresolver.clone())?;
        }

        // `inst_next` etc. has to be set at this point already since actions might use it
        resolver.set_inst_addresses(addr, addr + consumed_so_far, None);

        for (context_field_id, context_field_value) in context_stack.values() {
            if resolver.has_field(context_field_id) {
                trace_jir!("Defining context field value {context_field_id:?} := {context_field_value:?}");
                resolver.define_field(context_field_id, context_field_value)?;
            }
        }

        context_stack.push_state();

        for action in &constructor.actions {
            action.evaluate(&mut resolver, context_stack)?;
        }

        // sublists might consume bytes that the main match tree does not so we take the maximum
        let mut consumed_so_far_max = consumed_so_far;

        for offsetted_sublist in sublists {
            let sublist_bytes = &bytes[offsetted_sublist.offset..];

            let sublist = jumplist_cache
                .get(offsetted_sublist.jumplist)
                .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;
            trace_jir!(
                "Descending into offsetted sublist {:#?} with bytes={:#x?} (from {})",
                offsetted_sublist,
                sublist_bytes,
                constructor.display
            );

            let result = sublist.r#match(sublist_bytes, addr, jumplist_cache, constructors, context_stack)?;
            match result {
                MatcherResult::Some {
                    table: table_id,
                    resolver: sublist_resolver,
                    bytes_consumed: bytes_consumed_sublist,
                } => {
                    // sublist matches => add resolver
                    trace_jir!("Sublist consumed {} additional bytes", bytes_consumed_sublist);
                    trace_jir!("Adding subtable resolver {table_id:?}");
                    consumed_so_far_max = consumed_so_far_max.max(offsetted_sublist.offset + bytes_consumed_sublist);
                    resolver.define_subtable_resolver(table_id, sublist_resolver)?;
                }
                // sublist did not match => return
                _ => return Ok(result),
            }
        }

        // everything matched
        trace_jir!("Got a complete match");

        // TODO handle context flow
        context_stack.pop_state()?;

        return Ok(MatcherResult::Some {
            table: table_id,
            resolver,
            bytes_consumed: consumed_so_far_max,
        });
    }

    /// Match a given byte sequence into a constructor while resolving fields and setting context.
    /// The resulting [JIRResolver] contains all necessary information for building the final.
    ///
    /// # Return value
    ///
    /// - None if nothing was matched
    pub(crate) fn r#match<'a, 'b>(
        &'b self,
        bytes: &[u8],
        addr: EVAddr,
        jumplist_cache: &'b TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
    ) -> InternalCompilerResult<MatcherResult<'a>> {
        let mut accum = LocalMatching::default();
        self.match_local(bytes, addr, jumplist_cache, constructors, context_stack, 0, &mut accum)
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
