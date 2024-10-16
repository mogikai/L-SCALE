use chumsky::container::Container;
use itertools::Itertools;

use crate::compiler::ast::Spanned;
use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    MatcherId,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::spec::builder::{
    MOIRSymbolId,
    MOIRSymbolTables,
    StringId,
};
use crate::spec::jir::space::{
    JIRSpace,
    JIRSpaceType,
};
use crate::spec::jir::table::{
    JIRTable,
    Jumplist,
    JumplistContainer,
    JumplistEntry,
    JumplistId,
    NextJumplistEntry,
};
use crate::spec::jir::values::JIRVarNode;
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceType,
    SpaceId,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::{
    MOIRContext,
    MOIRContextId,
};
use crate::spec::moir::field::{
    FieldId,
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
use crate::spec::moir::varnode::{
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::typed_index_table::TypedIndexTable;
use crate::spec::{
    Ident,
    JIRSymbolId,
    JIRSymbolTables,
};
use std::collections::{
    HashMap,
    HashSet,
};

#[derive(Debug)]
pub(super) struct MOIRToJIRTransformer {
    moir: MOIRData,
    jir: JIRData,
}

#[derive(Debug)]
pub struct JIRData {
    pub symbol_ids: TypedIndexTable<Ident, JIRSymbolId>,
    pub symbol_strings: HashMap<Ident, StringId>,
    pub symbol_tables: JIRSymbolTables,
    pub string_to_ident_mapping: HashMap<StringId, Ident>,
}

#[derive(Debug)]
struct MOIRData {
    symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    string_to_ident_mapping: HashMap<StringId, Ident>,
    context: TypedIndexTable<MOIRContextId, MOIRContext>,
    root_table_id: TableId,
}

// TODO remove/move to somewhere else
#[derive(Debug)]
struct ResolvedMatcher {
    // matching
    token_byte_count: Vec<usize>,
    token_field_matchers: Vec<Vec<FieldMatcher>>,
    context_field_matchers: Vec<Vec<FieldMatcher>>,

    // extraction
    fields: Vec<Vec<FieldId>>,
    constructor_id: ConstructorId,
}

impl MOIRToJIRTransformer {
    pub fn new(
        symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
        symbol_strings: HashMap<Ident, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        string_to_ident_mapping: HashMap<StringId, Ident>,
        context: TypedIndexTable<MOIRContextId, MOIRContext>,
        root_table_id: TableId,
    ) -> Self {
        Self {
            moir: MOIRData {
                symbol_ids,
                symbol_strings,
                symbol_tables,
                string_to_ident_mapping,
                context,
                root_table_id,
            },
            jir: JIRData {
                symbol_ids: Default::default(),
                symbol_strings: Default::default(),
                symbol_tables: Default::default(),
                string_to_ident_mapping: Default::default(),
            },
        }
    }

    pub fn transform(mut self) -> SleighCompilerResult<JIRData> {
        self.jir.symbol_ids = self
            .moir
            .symbol_ids
            .into_iter()
            .map(|(ident, symbol_id)| {
                let symbol_id = match symbol_id {
                    MOIRSymbolId::Undefined => todo!("Generate error for undefined symbol"),
                    MOIRSymbolId::Macro(id) => JIRSymbolId::Macro(id),
                    MOIRSymbolId::Space(id) => JIRSymbolId::Space(id),
                    MOIRSymbolId::Table(id) => JIRSymbolId::Table(id),
                    MOIRSymbolId::Token(id) => JIRSymbolId::Token(id),
                    MOIRSymbolId::Field(id) => JIRSymbolId::Field(id),
                    MOIRSymbolId::VarNode(id) => JIRSymbolId::VarNode(id),
                    MOIRSymbolId::BitrangeRegister(id) => JIRSymbolId::BitrangeRegister(id),
                    MOIRSymbolId::CustomOp(id) => JIRSymbolId::CustomOp(id),
                };
                (ident, symbol_id)
            })
            .collect();
        self.jir.symbol_strings = self
            .moir
            .symbol_strings
            .into_iter()
            .map(|(k, v)| (k, v.into_value()))
            .collect();
        self.jir.symbol_tables = JIRSymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            tables: transform_tables(
                self.moir.symbol_tables.tables,
                &self.moir.root_table_id,
                &self.moir.symbol_tables.tokens,
                &self.moir.context,
                &self.moir.symbol_tables.varnodes,
                &self.moir.symbol_tables.constructors,
            )?,
            constructors: self.moir.symbol_tables.constructors,
            varnodes: transform_varnodes(self.moir.symbol_tables.varnodes)?,
        };
        self.jir.string_to_ident_mapping = self.moir.string_to_ident_mapping;
        Ok(self.jir)
    }
}

/// Transform tables by combining every root [Matcher] into multiple jumplists.
fn transform_tables(
    tables: TypedIndexTable<TableId, MOIRTable>,
    root_table_id: &TableId,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &TypedIndexTable<ConstructorId, MOIRConstructor>,
) -> SleighCompilerResult<TypedIndexTable<TableId, JIRTable>> {
    trace_build!("Processing {} tables into jumplists", tables.len());

    let mut jumplist_cache: TypedIndexTable<JumplistId, Jumplist> = TypedIndexTable::default();
    let mut sublist_cache: HashMap<(TableId, MOIRSize), JumplistId> = HashMap::new();
    let result: TypedIndexTable<TableId, JIRTable> = TypedIndexTable::default();

    let root_table = tables
        .get(*root_table_id)
        .ok_or_else(|| -> SleighCompilerError {
            todo!("Generate error report for missing root table");
        })?;
    let root_matchers = root_table.matchers.values().collect_vec();
    let jumplists = root_matchers
        .iter()
        .map(|matcher| {
            create_jumplist(
                0,
                matcher,
                &mut jumplist_cache,
                &mut sublist_cache,
                &tables,
                tokens,
                context,
                varnodes,
                constructors,
            )
        })
        .collect::<SleighCompilerResult<Vec<_>>>()?;

    trace_build!(
        "Created {} root jumplists ({} in total, {} cached sublists)",
        jumplists.len(),
        jumplist_cache.len(),
        sublist_cache.len()
    );

    let partitions = partition_jumplists_by_size(&jumplists, &jumplist_cache, tokens, context, varnodes)?;

    trace_build!("Partitioned root jumplists into {} partitions", partitions.len());

    let merged_jumplists = partitions
        .into_iter()
        .map(|(size, partitioned_jumplists)| {
            merge_jumplist_prefixes(size, &partitioned_jumplists, &mut jumplist_cache, tokens, context, varnodes)
        })
        .collect::<SleighCompilerResult<Vec<JumplistId>>>()?;

    trace_build!("Merge resulted in {} root jumplists", merged_jumplists.len());

    jumplist_cache.iter().for_each(|jumplist| {
        eprintln!("jumplist = {:#?}", jumplist);
    });
    merged_jumplists.iter().for_each(|jumplist_id| {
        let jumplist = jumplist_cache.get(*jumplist_id).unwrap();
        eprintln!("merged_jumplist[{:#?}] = {:#?}", jumplist_id, jumplist);
    });

    let result = TypedIndexTable::default();
    Ok(result)
}

fn create_jumplist(
    token_index: usize,
    matcher: &Matcher,
    jumplist_cache: &mut TypedIndexTable<JumplistId, Jumplist>,
    sublist_cache: &mut HashMap<(TableId, MOIRSize), JumplistId>,
    tables: &TypedIndexTable<TableId, MOIRTable>,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &TypedIndexTable<ConstructorId, MOIRConstructor>,
) -> SleighCompilerResult<JumplistId> {
    let constructor = constructors
        .get(
            matcher
                .constructor
                .expect("The constructor should be already set at this point"),
        )
        .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing constructor") })?;
    trace_build!("Creating jumplist for matcher = {:#?} (token_index={})", constructor.display, token_index);

    // create index by combining all field matchers into a single pattern and also create jumplists
    // for subtables
    let parent = matcher
        .token_list
        .list
        .get(token_index)
        .expect("There has to be a token left");
    let token_size = get_token_size(parent, tokens, context, varnodes)?;
    let mut mask = 0u64;
    let mut value = 0u64;
    let mut same_token_jumplists: Vec<JumplistId> = Vec::new();
    for atom in matcher
        .conjunctions
        .get(token_index)
        .expect("Mismatching token list and conjunction list size")
    {
        match atom {
            MatcherAtom::Field { field, matcher } => {
                if let FieldMatcher::Value {
                    mask: m_mask,
                    value: m_value,
                } = matcher
                {
                    mask |= *m_mask;
                    value |= *m_value;
                }
            }
            MatcherAtom::Submatchers { table, matchers: _ } => {
                let sublist = process_table(
                    *table,
                    token_size,
                    jumplist_cache,
                    sublist_cache,
                    tables,
                    tokens,
                    context,
                    varnodes,
                    constructors,
                )?;
                same_token_jumplists.push(sublist);
            }
        }
    }

    let next_entry = if token_index + 1 < matcher.token_list.list.len() {
        // at least one token left to process => create jumplist for the remainder
        NextJumplistEntry::Jumplist(create_jumplist(
            token_index + 1,
            matcher,
            jumplist_cache,
            sublist_cache,
            tables,
            tokens,
            context,
            varnodes,
            constructors,
        )?)
    } else {
        // no tokens left => terminate with matcher
        let display_string = constructors
            .get(
                matcher
                    .constructor
                    .expect("A constructor has to be assigned at this point"),
            )
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing constructor");
            })?
            .display
            .clone();
        NextJumplistEntry::Matcher(display_string)
    };

    let entry = JumplistEntry::new(same_token_jumplists, next_entry);

    let mut jumplist = Jumplist::new(mask, token_size);
    jumplist.insert(value, entry);
    let jumplist_id = jumplist_cache.insert(jumplist);

    Ok(jumplist_id)
}

/// Merge multiple jumplists of equal size by combining common prefixes. Subtables are not touched
/// and are processed instead by rolling lists up from the leaf nodes to the root.
fn merge_jumplist_prefixes(
    token_size: MOIRSize,
    jumplists: &[JumplistId],
    jumplist_cache: &mut TypedIndexTable<JumplistId, Jumplist>,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
) -> SleighCompilerResult<JumplistId> {
    // collect all entries in one list
    trace_build!("Merging jumplists {:?} with size {} by prefix", jumplists, token_size);

    let mut entry_map: HashMap<u64, Vec<JumplistEntry>> = HashMap::new();
    let mut mask = 0;
    for jumplist_id in jumplists.iter() {
        let jumplist = jumplist_cache
            .get(*jumplist_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing jumplist id") })?;
        if jumplist.bytes() != token_size {
            todo!("Generate error report for invalid jumplist list");
        }
        mask |= jumplist.mask();
        for (value, entries) in jumplist.entries() {
            entry_map
                .entry(value)
                .or_insert(Vec::new())
                .extend(entries.clone().into_iter());
        }
    }

    // combine entries in each slot of entry_map and insert into new jumplist
    let mut jumplist_new = Jumplist::new(mask, token_size);
    for (value, entries) in entry_map.into_iter() {
        // partition entries by same_token_jumplist; each partition is then reduced to a
        // single entry in case they have successors
        let entries_partitions = partition_entries_by_same_token_jumplist_and_next(entries, jumplist_cache)?;
        for entries_partition in entries_partitions.into_iter() {
            // collect successors; this is None if they match, but have no successors (i.e. all
            // entries terminate with the same matcher)
            let successors: Option<Vec<_>> = entries_partition
                .iter()
                .map(|entry| {
                    if let NextJumplistEntry::Jumplist(next) = entry.next_entry {
                        Some(next)
                    } else {
                        None
                    }
                })
                .collect();

            if let Some(successors) = successors {
                let same_token_jumplist = entries_partition
                    .first()
                    .expect("A partition must have an element")
                    .same_token_jumplist
                    .clone();

                // this partition has successors => partition successors, merge recursively and insert
                let successor_partitions =
                    partition_jumplists_by_size(&successors, jumplist_cache, tokens, context, varnodes)?;
                for (partition_size, successor_partition) in successor_partitions.into_iter() {
                    let merged = merge_jumplist_prefixes(
                        partition_size,
                        &successor_partition,
                        jumplist_cache,
                        tokens,
                        context,
                        varnodes,
                    )?;
                    let entry = JumplistEntry::new(same_token_jumplist.clone(), NextJumplistEntry::Jumplist(merged));
                    jumplist_new.insert(value, entry);
                }
            } else {
                // no successors => insert partition entries as-is
                entries_partition.into_iter().for_each(|entry| {
                    jumplist_new.insert(value, entry);
                })
            }
        }
    }

    // TODO sort entries by specificity (should this be done inside the jumplist?)
    Ok(jumplist_cache.insert(jumplist_new))
}

/// Partition jumplists by size. Necessary for merging since jumplists can only
/// be merged together if they are of equal size.
fn partition_jumplists_by_size(
    jumplists: &[JumplistId],
    jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
) -> SleighCompilerResult<Vec<(MOIRSize, Vec<JumplistId>)>> {
    // partition root matchers based on size of first token
    let mut partitions: Vec<(MOIRSize, Vec<JumplistId>)> = Vec::new();
    for jumplist_id in jumplists.iter() {
        let jumplist = jumplist_cache
            .get(*jumplist_id)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing jumplist");
            })?;
        let mut partition_found = false;
        for (partition_size, partition) in partitions.iter_mut() {
            if *partition_size == jumplist.bytes() {
                partition.push(*jumplist_id);
                partition_found = true;
                break;
            }
        }

        if !partition_found {
            partitions.push((jumplist.bytes(), vec![*jumplist_id]));
        }
    }

    Ok(partitions)
}

/// Partition entries by same_token_jumplist as well as the type of next_entry. Necessary for
/// merging since jumplist entries can only be merged together if their same_token_jumplist
/// matches and they all have a next_entry of type Jumplist with a common size.
fn partition_entries_by_same_token_jumplist_and_next<'a>(
    entries: Vec<JumplistEntry>,
    jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
) -> SleighCompilerResult<Vec<Vec<JumplistEntry>>> {
    // partition root matchers based on size of first token
    let mut partitions: Vec<Vec<JumplistEntry>> = Vec::new();
    for entry in entries.into_iter() {
        let mut partition_found = false;
        for partition in partitions.iter_mut() {
            let first = partition
                .first()
                .expect("If a partition was created it must contain an element");
            if next_entries_compatible(first, &entry, jumplist_cache)? {
                partition.push(entry.clone()); // TODO better way to prevent clone?
                partition_found = true;
                break;
            }
        }

        if !partition_found {
            partitions.push(vec![entry]);
        }
    }

    Ok(partitions)
}

/// Returns true if both entries have a common same_token_jumplist and their next entry
/// is either the same (in case of a matcher) or has the same size (in case of a jumplist).
fn next_entries_compatible(
    first: &JumplistEntry,
    second: &JumplistEntry,
    jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
) -> SleighCompilerResult<bool> {
    let jumplist_equal = first.same_token_jumplist == second.same_token_jumplist;
    let next_equal = first.next_entry == second.next_entry;
    let next_same_size = if let NextJumplistEntry::Jumplist(first_jlid) = first.next_entry {
        if let NextJumplistEntry::Jumplist(second_jlid) = second.next_entry {
            let first_jl = jumplist_cache
                .get(first_jlid)
                .ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing jumplist");
                })?;
            let second_jl = jumplist_cache
                .get(second_jlid)
                .ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing jumplist");
                })?;
            first_jl.bytes() == second_jl.bytes()
        } else {
            false
        }
    } else {
        false
    };

    Ok(jumplist_equal && (next_equal || next_same_size))
}

fn get_token_size(
    parent: &MOIRFieldParent,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
) -> SleighCompilerResult<MOIRSize> {
    match parent {
        MOIRFieldParent::Token(tid) => tokens
            .get(*tid)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing token");
            })
            .map(|token| token.bytes),
        MOIRFieldParent::Context(cid) => {
            let vnid = context
                .get(*cid)
                .ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing context");
                })?
                .varnode;
            let varnode = varnodes.get(vnid).ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing varnode");
            })?;
            Ok(varnode.size)
        }
    }
}

/// Process a table by creating and merging corresponding jumplists for each matcher of size
/// `token_size` in case the result hasn't been cached before.
fn process_table(
    table: TableId,
    token_size: MOIRSize,
    jumplist_cache: &mut TypedIndexTable<JumplistId, Jumplist>,
    sublist_cache: &mut HashMap<(TableId, MOIRSize), JumplistId>,
    tables: &TypedIndexTable<TableId, MOIRTable>,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &TypedIndexTable<ConstructorId, MOIRConstructor>,
) -> SleighCompilerResult<JumplistId> {
    trace_build!("Processing table {:?} with size {:?}", table, token_size);
    match sublist_cache.get(&(table, token_size)) {
        Some(jumplist) => {
            // table with corresponding token_size was already processed => link to previous result
            Ok(*jumplist)
        }
        None => {
            // no cached result exists => create jumplist for matchers with corresponding token size and store in sublist cache (we don't use the submatchers
            // field because tokens can be different as long as their sizes match)
            let subtable = tables.get(table).ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing table");
            })?;
            let sublists: SleighCompilerResult<Vec<_>> = subtable
                .matchers
                .values()
                .filter_map(|matcher| {
                    let parent = matcher
                        .token_list
                        .list
                        .get(0)
                        .expect("A matcher has to have at least one token");
                    match get_token_size(parent, tokens, context, varnodes) {
                        Ok(size) => {
                            if size == token_size {
                                let sublist = create_jumplist(
                                    0,
                                    matcher,
                                    jumplist_cache,
                                    sublist_cache,
                                    tables,
                                    tokens,
                                    context,
                                    varnodes,
                                    constructors,
                                );
                                Some(sublist)
                            } else {
                                None
                            }
                        }
                        Err(e) => Some(Err(e)),
                    }
                })
                .collect();

            // merge different matchers of same table
            let sublist = merge_jumplist_prefixes(token_size, &sublists?, jumplist_cache, tokens, context, varnodes)?;
            sublist_cache.insert((table, token_size), sublist);
            Ok(sublist)
        }
    }
}

fn transform_spaces(
    spaces: TypedIndexTable<SpaceId, MOIRSpace>
) -> SleighCompilerResult<TypedIndexTable<SpaceId, JIRSpace>> {
    Ok(spaces
        .into_iter()
        .map(|(ident, space)| {
            let r#type = match space.r#type {
                MOIRSpaceType::Ram => JIRSpaceType::Ram,
                MOIRSpaceType::Rom => JIRSpaceType::Rom,
                MOIRSpaceType::Register => JIRSpaceType::Register,
                MOIRSpaceType::Const => JIRSpaceType::Const,
            };
            let space = JIRSpace {
                r#type,
                address_size: space.address_size,
                wordsize: space.wordsize,
            };
            (ident, space)
        })
        .collect())
}

fn transform_varnodes(
    varnodes: TypedIndexTable<VarNodeId, MOIRVarNode>
) -> SleighCompilerResult<TypedIndexTable<VarNodeId, JIRVarNode>> {
    Ok(varnodes
        .into_iter()
        .map(|(ident, varnode)| {
            let varnode = JIRVarNode {
                space_id: varnode.space,
                offset: varnode.offset,
                size: varnode.size,
            };
            (ident, varnode)
        })
        .collect())
}
