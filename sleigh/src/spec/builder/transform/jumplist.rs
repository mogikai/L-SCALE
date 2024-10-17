use std::collections::{HashMap, HashSet};

use chumsky::container::Container;
use itertools::Itertools;

use crate::compiler::build::constructor::matcher::{FieldMatcher, Matcher, MatcherAtom};
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::spec::jir::table::{
    JIRTable, Jumplist, JumplistEntry, JumplistId, NextJumplistEntry
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
use crate::spec::moir::field::MOIRFieldParent;
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

/// Transforms [MOIRTable]s to [JIRTable]s by creating [Jumplist]s and other
/// related data structures.
pub(super) struct TableTransformer<'a> {
    jumplist_cache: TypedIndexTable<JumplistId, Jumplist>,
    sublist_cache: HashMap<(TableId, MOIRSize), JumplistId>,
    tables: &'a TypedIndexTable<TableId, MOIRTable>,
    tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
    context: &'a TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &'a TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
}

impl<'a> TableTransformer<'a> {
    pub(super) fn new(
        tables: &'a TypedIndexTable<TableId, MOIRTable>,
        tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
        context: &'a TypedIndexTable<MOIRContextId, MOIRContext>,
        varnodes: &'a TypedIndexTable<VarNodeId, MOIRVarNode>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
    ) -> Self {
        Self {
            jumplist_cache: TypedIndexTable::default(),
            sublist_cache: HashMap::new(),
            tables,
            tokens,
            context,
            varnodes,
            constructors,
        }
    }

    pub(super) fn transform(&mut self, root_table_id: TableId) -> SleighCompilerResult<TypedIndexTable<TableId, JIRTable>> {
        trace_build!("Processing {} tables into jumplists", self.tables.len());

        let root_table = self.tables
            .get(root_table_id)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing root table");
            })?;
        let root_matchers = root_table.matchers.values().collect_vec();
        let partition_sizes = root_matchers.into_iter().map(|matcher| {
            let parent = matcher.token_list.list.get(0).expect("A matcher has to have at least one token");
            self.get_token_size(parent)
        }).collect::<SleighCompilerResult<HashSet<MOIRSize>>>()?;

        let jumplists = partition_sizes.into_iter().map(|size| {
            self.process_table(root_table_id, size)
        }).collect::<SleighCompilerResult<Vec<JumplistId>>>()?;

        trace_build!(
            "Created {} root jumplists ({} in total, {} cached sublists)",
            jumplists.len(),
            self.jumplist_cache.len(),
            self.sublist_cache.len()
        );

        self.jumplist_cache.iter().for_each(|jumplist| {
            eprintln!("jumplist = {:#?}", jumplist);
        });
        jumplists.iter().for_each(|jumplist_id| {
            let jumplist = self.jumplist_cache.get(*jumplist_id).unwrap();
            eprintln!("merged_jumplist[{:#?}] = {:#?}", jumplist_id, jumplist);
        });

        let result = TypedIndexTable::default();
        Ok(result)
    }

    /// Process a table by creating and merging corresponding jumplists for each matcher of size
    /// `token_size` in case the result hasn't been cached before.
    fn process_table(
        &mut self,
        table: TableId,
        token_size: MOIRSize,
    ) -> SleighCompilerResult<JumplistId> {
        trace_build!("Processing table {:?} with size {:?}", table, token_size);
        match self.sublist_cache.get(&(table, token_size)) {
            Some(jumplist) => {
                // table with corresponding token_size was already processed => link to previous result
                Ok(*jumplist)
            }
            None => {
                // no cached result exists => create jumplist for matchers with corresponding token size and store in sublist cache (we don't use the submatchers
                // field because tokens can be different as long as their sizes match)
                let subtable = self.tables.get(table).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing table");
                })?;

                // get all correctly sized matchers
                let mut partition = Vec::new();
                for matcher in subtable.matchers.values() {
                    let parent = matcher.token_list.list.first().expect("A token list must have at least a single token");
                    let size = self.get_token_size(parent)?;
                    if size == token_size {
                        partition.push(matcher);
                    }
                }

                // create single jumplist out of them
                let jumplist = self.create_jumplist(0, &partition)?;

                // cache and return
                self.sublist_cache.insert((table, token_size), jumplist);
                Ok(jumplist)
            }
        }
    }

    /// Creates a single jumplist for multiple matchers with a common size.
    fn create_jumplist(
        &mut self,
        token_index: usize,
        matchers: &[&Matcher],
    ) -> SleighCompilerResult<JumplistId> {
        // each partition will be reduced to one entry which will then be merged into the final
        // jumplist
        let entry_partitions = self.partition_matchers_by_subtables_and_next(token_index, matchers)?;
        let entries = entry_partitions.into_iter().map(|partition| {
            let first = partition.first().expect("A partition must have a matcher");
            let size = self.get_token_size(first.token_list.list.get(token_index).expect("There must be a token"))?;

            let next_entry = if token_index + 1 < first.token_list.list.len() {
                // there are tokens left => process into jumplist and create entry
                let jumplist = self.create_jumplist(token_index + 1, &partition)?;
                NextJumplistEntry::Jumplist(jumplist)
            } else {
                // no tokens left => create entry
                let display = self.constructors.get(partition.first().unwrap().constructor.unwrap()).unwrap().display.clone();
                NextJumplistEntry::Matcher(display)
            };

            // collect subtables and mask/value
            let mut mask = 0u64;
            let mut value = 0u64;
            let mut sublists = Vec::new();
            for atom in first.conjunctions.get(token_index).expect("There must be a conjunction").iter() {
                match atom {
                    MatcherAtom::Field { field: _, matcher } => {
                        if let FieldMatcher::Value { mask: fmask, value: fvalue } = matcher {
                            mask |= *fmask;
                            value |= *fvalue;
                        }
                    }
                    MatcherAtom::Submatchers { table, matchers: _ } => {
                        let sublist = self.process_table(*table, size)?;
                        sublists.push(sublist);
                    }
                }
            }

            // create entry and return; the entry will be inserted at value in the new list
            let entry = JumplistEntry::new(sublists, next_entry);
            Ok((mask, value, entry))
        }).collect::<SleighCompilerResult<Vec<_>>>()?;

        // collect mask and create list
        let mut mask = 0u64;
        for (entry_mask, _, _) in entries.iter() {
            mask |= entry_mask
        }

        let size = self.get_token_size(matchers.get(0).expect("There has to be at least one matcher").token_list.list.get(token_index).expect("There has to be a token list"))?;
        let mut new_jumplist = Jumplist::new(mask, size);
        for (_, value, entry) in entries.into_iter() {
            new_jumplist.insert(value, entry);
        }

        // insert jumplist and return
        let new_jumplist_id = self.jumplist_cache.insert(new_jumplist);
        Ok(new_jumplist_id)
    }

    /// Partition matchers at `token_index in such a way that each partition can be combined into
    /// a single jumplist. This is the case if the matchers share a common token size since
    /// the jumplist needs to consume the same number of bytes. The endianness might be
    /// different though since this can be handled by adjusting the values themselves.
    fn partition_matchers_by_size(
        &self,
        token_index: usize,
        matchers: &[&'a Matcher],
    ) -> SleighCompilerResult<Vec<(MOIRSize, Vec<&'a Matcher>)>> {
        // partition root matchers based on size of first token
        let mut partitions: Vec<(MOIRSize, Vec<&'a Matcher>)> = Vec::new();
        for matcher in matchers.iter() {
            let matcher_size = if let Some(parent) = matcher.token_list.list.get(token_index) {
                self.get_token_size(parent)?
            } else {
                continue;
            };

            let mut partition_found = false;
            for (partition_size, partition) in partitions.iter_mut() {
                if *partition_size == matcher_size {
                    partition.push(matcher);
                    partition_found = true;
                    break;
                }
            }

            if !partition_found {
                partitions.push((matcher_size, vec![matcher]));
            }
        }

        Ok(partitions)
    }

    /// Partition matchers at `token_index in such a way that all entries of each partition can be combined into
    /// a single entry. This is the case if
    /// 1. The matchers share common subtables, and
    /// 2. The matchers either
    ///     2.1 both terminate and share a common constructor, or
    ///     2.2 both have a next token with equal size
    fn partition_matchers_by_subtables_and_next(
        &self,
        token_index: usize,
        matchers: &[&'a Matcher],
    ) -> SleighCompilerResult<Vec<Vec<&'a Matcher>>> {
        // partition root matchers based on size of first token
        let mut partitions: Vec<Vec<&'a Matcher>> = Vec::new();
        for matcher in matchers.iter() {
            let mut partition_found = false;
            for partition in partitions.iter_mut() {
                let first = partition.first().expect("If there is a partition it must contain an element");
                if self.matchers_compatible(token_index, first, matcher)? {
                    partition.push(matcher);
                    partition_found = true;
                    break;
                }
            }

            if !partition_found {
                partitions.push(vec![matcher]);
            }
        }

        Ok(partitions)
    }

    /// Checks the condition for [partition_matchers_by_subtables_and_next]
    fn matchers_compatible(&self, token_index: usize, lhs: &Matcher, rhs: &Matcher) -> SleighCompilerResult<bool> {
        // check for same subtables
        let clhs = lhs.conjunctions.get(token_index).expect("There needs to be a conjunction at token_index");
        let crhs = rhs.conjunctions.get(token_index).expect("There needs to be a conjunction at token_index");
        for atom in clhs {
            if let MatcherAtom::Submatchers { table: table_lhs, matchers: _ } = atom {
                if crhs.iter().find(|atom| if let MatcherAtom::Submatchers { table: table_rhs, matchers: _ } = atom { table_lhs == table_rhs } else { false }).is_none() {
                    return Ok(false);
                }
            }
        }

        if let Some(lhs_parent) = lhs.token_list.list.get(token_index + 1) {
            if let Some(rhs_parent) = rhs.token_list.list.get(token_index + 1) {
                // both have next tokens => compatible?
                let lhs_size = self.get_token_size(lhs_parent)?;
                let rhs_size = self.get_token_size(rhs_parent)?;
                Ok(lhs_size == rhs_size)
            } else {
                // lhs is some but rhs is none
                Ok(false)
            }
        } else {
            // lhs is none and rhs is none
            Ok(rhs.token_list.list.get(token_index + 1).is_none())
        }
    }

    fn get_token_size(
        &self,
        parent: &MOIRFieldParent,
    ) -> SleighCompilerResult<MOIRSize> {
        match parent {
            MOIRFieldParent::Token(tid) => self.tokens
                .get(*tid)
                .ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing token");
                })
            .map(|token| token.bytes),
            MOIRFieldParent::Context(cid) => {
                let vnid = self.context
                    .get(*cid)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing context");
                    })?
                .varnode;
                let varnode = self.varnodes.get(vnid).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing varnode");
                })?;
                Ok(varnode.size)
            }
        }
    }
}
