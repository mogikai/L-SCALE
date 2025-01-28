use mogi_utilities::addresses::EVAddr;
use rustc_hash::FxHashMap;

use crate::compiler::InternalCompilerResult;
use crate::compiler::error::InternalError;
use crate::new_typed_index_table_id;
use crate::spec::ProcessorSpecification;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::JIRConst;
use crate::spec::moir::actions::MOIRActionTarget;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::MOIRContextId;
use crate::spec::moir::field::{
    FieldId,
    MOIRBitRange,
};
use crate::spec::moir::table::TableId;
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
    sublists: Vec<&'a Vec<JumplistId>>,
    bytes: Vec<&'a [u8]>,
    constructor_id: ConstructorId,
    table_id: TableId,
    extracted_fields: FxHashMap<FieldId, u64>,
}

impl<'a> LocalMatching<'a> {
    fn new(
        sublists: Vec<&'a Vec<JumplistId>>,
        bytes: Vec<&'a [u8]>,
        constructor_id: ConstructorId,
        table_id: TableId,
    ) -> Self {
        Self {
            sublists,
            bytes,
            constructor_id,
            table_id,
            extracted_fields: FxHashMap::default(),
        }
    }
}

/// Since we don't know where exactly fields are located in the byte stream, we
/// need to store this information in each jumplist so we can reconstruct the field
/// values later on.
#[derive(Debug, Clone)]
pub(crate) struct FieldExtractor {
    field_id: FieldId,
    bits: MOIRBitRange,
}

impl FieldExtractor {
    pub(crate) fn new(
        field_id: FieldId,
        bits: MOIRBitRange,
    ) -> Self {
        Self { field_id, bits }
    }

    /// Extract parts of the field from `token_value` and insert into `fields`. Might need to be
    /// called multiple times since fields that cross byte boundaries can be fragmented across
    /// multiple jumplists.
    pub(crate) fn process_value(
        &self,
        token_value: u64,
        fields: &mut FxHashMap<FieldId, u64>,
    ) {
        let field_mut = fields.entry(self.field_id).or_default();
        *field_mut |= self.bits.extract(token_value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum NextJumplistEntry {
    // consume self.bytes bytes and continue with next
    TrailingNext(JumplistId),
    // don't consume any bytes and continue with next
    TrailingSame(JumplistId),
    Constructor { cid: ConstructorId, tid: TableId },
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
    Large(FxHashMap<u64, Vec<JumplistEntry>>),
}

impl Default for JumplistContainer {
    fn default() -> Self {
        JumplistContainer::Large(FxHashMap::default())
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
    mask: u64, // usually only a few bits are actually relevant for indexing into the list
    bytes: MOIRSize,
    data_provider: DataProvider,
    field_extractors: FxHashMap<ConstructorId, Vec<FieldExtractor>>,
}

impl Jumplist {
    pub(crate) fn new(
        mask: u64,
        bytes: MOIRSize,
        data_provider: DataProvider,
        field_extractors: FxHashMap<ConstructorId, Vec<FieldExtractor>>,
    ) -> Self {
        let container = if bytes <= 1 {
            JumplistContainer::Small([const { None }; 256])
        } else {
            JumplistContainer::Large(FxHashMap::default())
        };

        Self {
            container,
            mask,
            bytes,
            data_provider,
            field_extractors,
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
        context_provider: &ContextProvider,
    ) -> InternalCompilerResult<Option<(u64, &'a [u8])>> {
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

                        let value = u64::from_le_bytes(value_byte_arr);
                        Ok(Some((value, &bytes[usize::from(self.bytes)..])))
                    }
                    None => Ok(None), // no bytes left
                }
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
    fn match_local<'a>(
        &'a self,
        bytes: &'a [u8],
        jumplist_cache: &'a TypedIndexTable<JumplistId, Jumplist>,
        context_provider: &ContextProvider,
    ) -> InternalCompilerResult<Vec<LocalMatching<'a>>> {
        // field extraction ignores the mask so we need the unmasked value as well
        let (value_unmasked, remaining_bytes) = self
            .decode_value(bytes, context_provider)?
            .expect("Couldn't decode value");

        let value_masked = value_unmasked & self.mask;

        let mut lms = Vec::new();

        if let Some(entries) = self.get(value_masked)? {
            for entry in entries {
                let mut next_lms = match &entry.next_entry {
                    NextJumplistEntry::TrailingNext(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        jumplist.match_local(remaining_bytes, jumplist_cache, context_provider)?
                    }
                    NextJumplistEntry::TrailingSame(jlid) => {
                        let jumplist = jumplist_cache
                            .get(*jlid)
                            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;

                        jumplist.match_local(bytes, jumplist_cache, context_provider)?
                    }
                    NextJumplistEntry::Constructor { cid, tid } => {
                        trace_jir!("Reached leaf; cid={cid:?}, tid={tid:?}");
                        vec![LocalMatching::new(Vec::new(), Vec::new(), *cid, *tid)]
                    }
                };

                for next_lm in &mut next_lms {
                    // extract fields and update local matching
                    if let Some(field_extractors) = self.field_extractors.get(&next_lm.constructor_id) {
                        for field_extractor in field_extractors {
                            field_extractor.process_value(value_unmasked, &mut next_lm.extracted_fields);
                        }
                    }

                    // add sublists of current entry
                    next_lm.sublists.push(&entry.sublists);
                    next_lm.bytes.push(bytes);
                }

                // add to existing matchings
                lms.append(&mut next_lms);
            }
        }

        // this is empty if nothing was found
        Ok(lms)
    }

    // TODO correct return type
    pub(crate) fn r#match<'a>(
        &'a self,
        bytes: &[u8],
        jumplist_cache: &'a TypedIndexTable<JumplistId, Jumplist>,
        spec: &'a ProcessorSpecification,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_provider: &mut ContextProvider,
    ) -> InternalCompilerResult<Option<(TableId, JIRResolver<'a>)>> {
        // TODO handle end of byte stream
        // let (value, remaining_bytes) = self.decode_value(bytes, context)?.expect("Couldn't decode value");

        // match next entry while skipping sublists while extracting fields
        let local_matchings = self.match_local(bytes, jumplist_cache, context_provider)?;

        // entry matches with constructor => extract fields, execute actions, save values
        // and descent into sublists

        'next_candidate: for local_matching in local_matchings {
            let constructor = constructors
                .get(local_matching.constructor_id)
                .ok_or_else(|| -> InternalError {
                    todo!("Generate error report for missing constructor");
                })?;

            trace_jir!("Got constructor {}", constructor.display);

            // the resolver with all local values is needed for the actions so we
            // instantiate it here
            let mut resolver = JIRResolver::new(
                spec,
                constructor,
                EVAddr::from(0xDEADBEEFu64),
                EVAddr::from(0xDEADBEEFu64),
                Some(EVAddr::from(0xDEADBEEFu64)),
            );

            for (field_id, field_value) in local_matching.extracted_fields.into_iter() {
                trace_jir!("Defining field value {field_id:?} := {field_value:?}");
                resolver.define_field(field_id, field_value)?;
            }

            for (context_field_id, context_field_value) in context_provider.values() {
                if resolver.has_field(context_field_id) {
                    trace_jir!("Defining context field value {context_field_id:?} := {context_field_value:?}");
                    resolver.define_field(context_field_id, context_field_value)?;
                }
            }

            context_provider.push_state();

            for action in &constructor.actions {
                let value = action.evaluate(&resolver)?;
                match action.lhs {
                    MOIRActionTarget::ContextField(field_id) => {
                        trace_jir!("Action is setting field {} := {}", field_id, value);
                        context_provider.set_field_value(field_id, value)?;
                        resolver.define_field(field_id, value)?;
                    }
                    // TODO implement temporaries
                    MOIRActionTarget::Local(local_id) => {
                        trace_jir!("Action is setting local {} := {}", local_id, value);
                        resolver.define_id(local_id, JIRValue::Const(JIRConst::new(value.into(), 0)));
                    }
                }
            }

            // TODO what about the bytes consumed by the sublists? this doesn't really work since we
            // can't delay that to the end since the amount of bytes consumed affects later matching
            // (there are also some recursive dependencies when the sublist matching itself
            // depends on a context variable set to a suffix of the sublist)
            let mut sublist_resolvers = Vec::new();
            for (sublist_ids, sublist_bytes) in local_matching
                .sublists
                .into_iter()
                .zip(local_matching.bytes.into_iter())
            {
                for sublist_id in sublist_ids {
                    let sublist = jumplist_cache
                        .get(*sublist_id)
                        .ok_or_else(|| -> InternalError { todo!("Generate error report for missing jumplist") })?;
                    trace_jir!(
                        "Descending into sublist {:#?} with bytes={:?} (from {})",
                        sublist_id,
                        sublist_bytes,
                        constructor.display
                    );
                    let (table_id, resolver) =
                        match sublist.r#match(sublist_bytes, jumplist_cache, spec, constructors, context_provider)? {
                            Some(resolver) => resolver,
                            None => {
                                // no match => try next
                                trace_jir!("No match, trying next (if existing)");
                                context_provider.pop_state()?;
                                continue 'next_candidate;
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

            return Ok(Some((local_matching.table_id, resolver)));
        }

        Ok(None)
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
