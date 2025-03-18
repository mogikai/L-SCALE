use std::cell::Cell;

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
/// To avoid traversing the hierarchy twice, we store necessary information here like
/// the remaining sublists.
#[derive(Debug, Default)]
struct MatchState<'a> {
    // collect sublist resolvers (no sublists since we immediately descend) and values with a
    // dynamic offset and store as vec for backtracking
    subresolvers: Vec<(TableId, JIRResolver<'a>)>,
    // need to store all extractor maps with the corresponding byte index since we
    // don't know the constructor id at this point
    field_extractors: Vec<(usize, &'a FnvHashMap<ConstructorId, Vec<FieldExtractor>>)>,
}

#[derive(Debug)]
pub enum MatcherResult<'a> {
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
    pub(crate) jumplist: JumplistId,
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
    pub(crate) field_extractors: FnvHashMap<ConstructorId, Vec<FieldExtractor>>,
    // we count the number of hits for each entry and rearrange later
    hit_count: Cell<u64>,
}

impl JumplistEntry {
    pub(crate) fn new(
        next_entry: NextJumplistEntry,
        sublists: Vec<JumplistId>,
        field_extractors: FnvHashMap<ConstructorId, Vec<FieldExtractor>>,
    ) -> Self {
        Self {
            next_entry,
            sublists,
            field_extractors,
            hit_count: Cell::from(0),
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
pub struct Jumplist {
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
    fn match_local<'a>(
        &'a self,
        bytes: &'a [u8],
        addr: EVAddr,
        jumplist_cache: &'a TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
        consumed_so_far: usize,
        state: &mut MatchState<'a>,
    ) -> InternalCompilerResult<MatcherResult<'a>> {
        let (value_masked, consumed_decoded) = self
            .decode_value(bytes, consumed_so_far, context_stack)?
            .expect("Couldn't decode value");

        if let Some(entries) = self.get(value_masked)? {
            let mut consumed_total = consumed_so_far + consumed_decoded;

            'next_entry: for entry in entries {
                // store state for backtracking
                let pl_subresolvers = state.subresolvers.len();
                entry.hit_count.set(entry.hit_count.get() + 1);

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
                            bytes_consumed: consumed_jumplist,
                        } => {
                            // if we have dynamic sublists, they dictate how much is consumed
                            consumed_total = consumed_total.max(consumed_so_far + consumed_jumplist);
                            state.subresolvers.push((table, resolver));
                        }
                    }
                }

                // variable-offset extractors will be processed later when the constructor id is
                // known
                state
                    .field_extractors
                    .push((consumed_so_far, &entry.field_extractors));

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
                            consumed_total,
                            state,
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
                            consumed_total,
                            state,
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
                            state,
                        )?
                    }
                };

                match result {
                    MatcherResult::None => {
                        // no complete match => backtrack and try next entry
                        state.subresolvers.truncate(pl_subresolvers);
                        state.field_extractors.pop();
                        entry.hit_count.set(entry.hit_count.get() - 1);
                    }
                    // complete match or end of stream => return
                    _ => return Ok(result),
                }
            }
        }

        // no match
        Ok(MatcherResult::None)
    }

    fn process_local_matching<'a>(
        &'a self,
        bytes: &'a [u8],
        addr: EVAddr,
        jumplist_cache: &'a TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
        consumed_so_far: usize,
        state: &MatchState<'a>,
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

        // variable-offset field extractors
        for (offset, field_extractor_map) in &state.field_extractors {
            if let Some(field_extractors) = field_extractor_map.get(&constructor_id) {
                for field_extractor in field_extractors {
                    trace_jir!(
                        "Extracting local {} with entry extractor {:#?}",
                        field_extractor.local_id(),
                        field_extractor
                    );
                    let extracted_value = field_extractor.extract_value(&bytes[*offset..])?;
                    values[field_extractor.local_id()] = extracted_value;
                }
            }
        }

        // extract token and context fields with fixed offset into map
        for field_extractor in field_extractors {
            let extracted_value = field_extractor.extract_value(bytes)?;
            trace_jir!("Field extractor is defining {} := {:#?}", field_extractor.local_id(), extracted_value);
            values[field_extractor.local_id()] = extracted_value;
        }

        // the resolver with all local values is needed for the actions so we
        // instantiate it here
        let mut resolver = JIRResolver::new(constructor, constructor_id, values);

        for (subtable, subresolver) in &state.subresolvers {
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
    pub fn r#match<'a>(
        &'a self,
        bytes: &'a [u8],
        addr: EVAddr,
        jumplist_cache: &'a TypedIndexTable<JumplistId, Jumplist>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
        context_stack: &mut ContextStack,
    ) -> InternalCompilerResult<MatcherResult<'a>> {
        let mut accum = MatchState::default();
        self.match_local(bytes, addr, jumplist_cache, constructors, context_stack, 0, &mut accum)
    }

    /// Each entry stores the number of times it was part of a matching branch in
    /// [JumplistEntry::hit_count]. This function sorts the entries by their hit count in
    /// descending order and subsequently resets the hit counts to zero.
    pub fn rearrange_entries(&mut self) {
        for (_, entry_vec) in self.entries_mut() {
            entry_vec.sort_by(|e0, e1| e0.hit_count.get().cmp(&e1.hit_count.get()).reverse());
        }
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

mod test {
    use std::path::Path;

    use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;

    use super::*;
    use crate::compiler::SleighCompilerResult;
    use crate::compiler::builder::SleighCompilerBuilder;
    use crate::lifter::context_provider::{
        ContextProvider,
        SimpleContextProvider,
    };
    use crate::spec::ProcessorSpecification;
    use crate::spec::jir::context_stack::ContextStack;
    use crate::spec::moir::constructor::ConstructorId;
    use crate::spec::moir::table::TableId;

    const ASSETS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/test/");

    fn build(path: String) -> SleighCompilerResult<ProcessorSpecification> {
        let spec_path = Path::new(&path);
        let spec_dir = spec_path.parent().unwrap();
        let factory = SleighPreprocessorFactory::new().working_dir(spec_dir.to_path_buf());
        let compiler = SleighCompilerBuilder::new()
            .preprocessor_factory(factory)
            .build();
        compiler.compile(spec_path)
    }

    #[test]
    fn test_pdep() {
        let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
        let spec = build(path).unwrap();

        let mut context_provider = SimpleContextProvider::default();
        context_provider.set(&spec, "longMode", 1);
        context_provider.set(&spec, "addrsize", 2);
        context_provider.set(&spec, "opsize", 1);

        let root_jumplist = spec.jumplist_cache().values().next().unwrap();
        // pdep rax, rcx, [rbp+8*rsi+0x1234]
        let bytes = b"\xc4\xe2\xf3\xf5\x84\xf5\xcc\xed\xff\xff";

        let mut context_stack = ContextStack::new(&spec, &context_provider).expect("Couldn't create context stack");
        let MatcherResult::Some {
            table,
            resolver,
            bytes_consumed,
        } = root_jumplist
            .r#match(bytes, 0x0u64.into(), spec.jumplist_cache(), spec.constructors(), &mut context_stack)
            .unwrap()
        else {
            panic!("No match");
        };

        let tid_instr: TableId = 0.into();
        let tid_reg64: TableId = 4.into();
        let tid_rm64: TableId = 120.into();
        let tid_mem: TableId = 46.into();
        let tid_addr64: TableId = 41.into();
        let tid_index64: TableId = 15.into();
        let tid_base64: TableId = 14.into();
        let tid_simm32_64: TableId = 31.into();

        let cid_instr: ConstructorId = 456.into();
        let cid_pdep: ConstructorId = 4781.into();
        let cid_reg64: ConstructorId = 7.into();
        let cid_rm64: ConstructorId = 362.into();
        let cid_mem: ConstructorId = 198.into();
        let cid_addr64: ConstructorId = 168.into();
        let cid_index: ConstructorId = 32.into();
        let cid_base: ConstructorId = 30.into();
        let cid_simm32_64: ConstructorId = 59.into();

        assert_eq!(resolver.constructor_id(), cid_instr);
        let res_pdep = resolver.subtables().get(&tid_instr).unwrap();
        assert_eq!(res_pdep.constructor_id(), cid_pdep);
        let res_reg64 = res_pdep.subtables().get(&tid_reg64).unwrap();
        assert_eq!(res_reg64.constructor_id(), cid_reg64);
        let res_rm64 = res_pdep.subtables().get(&tid_rm64).unwrap();
        assert_eq!(res_rm64.constructor_id(), cid_rm64);
        let res_mem = res_rm64.subtables().get(&tid_mem).unwrap();
        assert_eq!(res_mem.constructor_id(), cid_mem);
        let res_addr64 = res_mem.subtables().get(&tid_addr64).unwrap();
        assert_eq!(res_addr64.constructor_id(), cid_addr64);
        let res_index = res_addr64.subtables().get(&tid_index64).unwrap();
        assert_eq!(res_index.constructor_id(), cid_index);
        let res_base = res_addr64.subtables().get(&tid_base64).unwrap();
        assert_eq!(res_base.constructor_id(), cid_base);
        let res_simm32_64 = res_addr64.subtables().get(&tid_simm32_64).unwrap();
        assert_eq!(res_simm32_64.constructor_id(), cid_simm32_64);
    }

    #[test]
    fn test_cmovge() {
        let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
        let spec = build(path).unwrap();

        let mut context_provider = SimpleContextProvider::default();
        context_provider.set(&spec, "longMode", 1);
        context_provider.set(&spec, "addrsize", 2);
        context_provider.set(&spec, "opsize", 1);

        let root_jumplist = spec.jumplist_cache().values().next().unwrap();
        // cmovge rcx, r12
        let bytes = b"\x49\x0F\x4D\xCC";

        let mut context_stack = ContextStack::new(&spec, &context_provider).expect("Couldn't create context stack");
        let MatcherResult::Some {
            table,
            resolver,
            bytes_consumed,
        } = root_jumplist
            .r#match(bytes, 0x0u64.into(), spec.jumplist_cache(), spec.constructors(), &mut context_stack)
            .unwrap()
        else {
            panic!("No match");
        };

        let tid_instr: TableId = 0.into();
        let tid_cc: TableId = 131.into();
        let tid_rm64: TableId = 120.into();
        let tid_rmr64: TableId = 11.into();
        let tid_reg64: TableId = 4.into();

        let cid_instr: ConstructorId = 449.into();
        let cid_cmov: ConstructorId = 628.into();
        let cid_cc: ConstructorId = 391.into();
        let cid_rm64: ConstructorId = 361.into();
        let cid_rmr64: ConstructorId = 25.into();
        let cid_reg64: ConstructorId = 7.into();

        assert_eq!(resolver.constructor_id(), cid_instr);
        let res_cmov = resolver.subtables().get(&tid_instr).unwrap();
        assert_eq!(res_cmov.constructor_id(), cid_cmov);
        let res_cc = res_cmov.subtables().get(&tid_cc).unwrap();
        assert_eq!(res_cc.constructor_id(), cid_cc);
        let res_rm64 = res_cmov.subtables().get(&tid_rm64).unwrap();
        assert_eq!(res_rm64.constructor_id(), cid_rm64);
        let res_rmr64 = res_rm64.subtables().get(&tid_rmr64).unwrap();
        assert_eq!(res_rmr64.constructor_id(), cid_rmr64);
        let res_reg64 = res_cmov.subtables().get(&tid_reg64).unwrap();
        assert_eq!(res_reg64.constructor_id(), cid_reg64);
    }

    #[test]
    fn test_vfmaddsub213pd() {
        let path = ASSETS_PATH.to_string() + "x86/x86-64.slaspec";
        let spec = build(path).unwrap();

        let mut context_provider = SimpleContextProvider::default();
        context_provider.set(&spec, "longMode", 1);
        context_provider.set(&spec, "addrsize", 2);
        context_provider.set(&spec, "opsize", 1);

        let root_jumplist = spec.jumplist_cache().values().next().unwrap();
        // vfmaddsub213pd xmm4, xmm1, xmmword ptr [0x1337]
        let bytes = b"\xC4\xE2\xF1\xA6\x24\x25\x37\x13\x00\x00";

        let mut context_stack = ContextStack::new(&spec, &context_provider).expect("Couldn't create context stack");
        let MatcherResult::Some {
            table,
            resolver,
            bytes_consumed,
        } = root_jumplist
            .r#match(bytes, 0x0u64.into(), spec.jumplist_cache(), spec.constructors(), &mut context_stack)
            .unwrap()
        else {
            panic!("No match");
        };

        let tid_instr: TableId = 0.into();
        let tid_xmmreg1: TableId = 17.into();
        let tid_zmmreg1: TableId = 21.into();
        let tid_zmmreg1: TableId = 21.into();
        let tid_xmmreg2_m128: TableId = 84.into();
        let tid_m128: TableId = 54.into();
        let tid_mem: TableId = 46.into();
        let tid_addr64: TableId = 41.into();
        let tid_simm32_64: TableId = 31.into();

        let cid_instr: ConstructorId = 454.into();
        let cid_vfmaddsub213pd: ConstructorId = 4841.into();
        let cid_xmmreg1: ConstructorId = 36.into();
        let cid_zmmreg1: ConstructorId = 44.into();
        let cid_xmmreg2_m128: ConstructorId = 271.into();
        let cid_m128: ConstructorId = 209.into();
        let cid_mem: ConstructorId = 198.into();
        let cid_addr64: ConstructorId = 160.into();
        let cid_simm32_64: ConstructorId = 59.into();

        assert_eq!(resolver.constructor_id(), cid_instr);
        let res_vfmaddsub213pd = resolver.subtables().get(&tid_instr).unwrap();
        assert_eq!(res_vfmaddsub213pd.constructor_id(), cid_vfmaddsub213pd);
        let res_xmmreg1 = res_vfmaddsub213pd.subtables().get(&tid_xmmreg1).unwrap();
        assert_eq!(res_xmmreg1.constructor_id(), cid_xmmreg1);
        let res_zmmreg1 = res_vfmaddsub213pd.subtables().get(&tid_zmmreg1).unwrap();
        assert_eq!(res_zmmreg1.constructor_id(), cid_zmmreg1);
        let res_xmmreg2_m128 = res_vfmaddsub213pd.subtables().get(&tid_xmmreg2_m128).unwrap();
        assert_eq!(res_xmmreg2_m128.constructor_id(), cid_xmmreg2_m128);
        let res_m128 = res_xmmreg2_m128.subtables().get(&tid_m128).unwrap();
        assert_eq!(res_m128.constructor_id(), cid_m128);
        let res_mem = res_m128.subtables().get(&tid_mem).unwrap();
        assert_eq!(res_mem.constructor_id(), cid_mem);
        let res_addr64 = res_mem.subtables().get(&tid_addr64).unwrap();
        assert_eq!(res_addr64.constructor_id(), cid_addr64);
        let res_simm32_64 = res_addr64.subtables().get(&tid_simm32_64).unwrap();
        assert_eq!(res_simm32_64.constructor_id(), cid_simm32_64);
    }
}
