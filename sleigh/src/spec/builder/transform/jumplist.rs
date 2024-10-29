use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use chumsky::container::{Container, Seq};
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
use crate::spec::moir::field::{MOIRBitRange, MOIRFieldParent};
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
    sublist_cache: HashMap<TableId, JumplistId>,
    tables: &'a TypedIndexTable<TableId, MOIRTable>,
    tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
    context: &'a TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &'a TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
}

// Temporary data used for processing tokens and bytes
struct Row<'a> {
    pub matcher: &'a Matcher,
    pub mask: u64,
    pub value: u64,
    pub token_size: MOIRSize,
    pub sublists: Vec<JumplistId>,
}

impl<'a> Row<'a> {
    fn new(matcher: &'a Matcher, mask: u64, value: u64, token_size: MOIRSize, sublists: Vec<JumplistId>) -> Self { Self { matcher, mask, value, token_size, sublists } }
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
        let root_jumplist_id = self.process_table(root_table_id)?;

        trace_build!(
            "Created root jumplist ({} in total, {} cached sublists)",
            self.jumplist_cache.len(),
            self.sublist_cache.len(),
        );

        self.jumplist_cache.iter().filter(|(id, _)| *id != root_jumplist_id).for_each(|jumplist| {
            eprintln!("jumplist = {:#?}", jumplist);
        });

        let root_jumplist = self.jumplist_cache.get(root_jumplist_id).expect("We just created this");
        eprintln!("root_jumplist = {:#?}", (root_jumplist_id, root_jumplist));

        let result = TypedIndexTable::default();
        Ok(result)
    }

    /// Process a table by creating and merging corresponding jumplists for each matcher of size
    /// `token_size` in case the result hasn't been cached before.
    /// # Preconditions:
    /// - there must be at least one matcher associated with `table` with a corresponding first token size of `token_size`
    fn process_table(
        &mut self,
        table: TableId,
    ) -> SleighCompilerResult<JumplistId> {
        match self.sublist_cache.get(&table) {
            Some(jumplist) => {
                // table with was already processed => link to previous result
                Ok(*jumplist)
            }
            None => {
                // no cached result exists => create jumplist for matchers with corresponding token size and store in sublist cache (we don't use the submatchers
                // field because tokens can be different as long as their sizes match)
                trace_build!("Processing table {:?}", table);

                let subtable = self.tables.get(table).ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing table");
                })?;

                let matchers = subtable.matchers.values().collect_vec();

                // create single jumplist out of all matchers
                let jumplist = self.create_jumplist(0, &matchers)?;

                // cache and return
                self.sublist_cache.insert(table, jumplist);
                Ok(jumplist)
            }
        }
    }

    /// Create a single jumplist for multiple matchers.
    ///
    /// # Preconditions:
    /// - `matchers` must not be empty
    /// - each matcher must have a token list of length at least `token_index` + 1
    fn create_jumplist(
        &mut self,
        token_index: usize,
        matchers: &[&Matcher],
    ) -> SleighCompilerResult<JumplistId> {
        // 1. extract everything necessary for token processing
        let rows = matchers.iter().map(|matcher| {
            let conjunction = matcher.conjunctions.get(token_index).expect("There must be a conjunction at token_index");
            let (mask, value, subtables) = extract_from_conjunction(&conjunction);
            let parent = matcher.token_list.list.get(token_index).expect("There must be a token at token_index");
            let token_size = self.get_token_size(parent)?;

            let sublists = subtables.into_iter().map(|subtable| {
                let cached = self.sublist_cache.get(&subtable);
                match cached {
                    Some(sublist) => Ok(*sublist),
                    None => {
                        let sublist = self.process_table(subtable)?;
                        Ok(sublist)
                    }
                }
            }).collect::<SleighCompilerResult<Vec<_>>>()?;

            Ok(Row::new(matcher, mask, value, token_size, sublists))
        }).collect::<SleighCompilerResult<Vec<_>>>()?;

        // 2. pass to recursive function
        self.create_jumplist_bytes(token_index, rows, 0)
    }

    /// Recursion used for processing tokens of different length into a single
    /// jumplist by splitting larger tokens into the least common size and then
    /// building jumplists on top of that.
    ///
    /// # Preconditions
    /// - `rows` must not be empty`
    /// # Return on success
    ///  - the new jumplist id
    fn create_jumplist_bytes(&mut self, token_index: usize, rows: Vec<Row>, bytes_consumed: MOIRSize) -> SleighCompilerResult<JumplistId> {
        assert_ne!(rows.len(), 0, "There must be rows");

        // new size is equal to size of smallest row
        let size_delta = rows.iter().map(|row| row.token_size).min().expect("We checked for this") - bytes_consumed;
        let size_bits = MOIRBitRange::new(u8::try_from(bytes_consumed * 8).expect("This should be 64 at most"), u8::try_from((bytes_consumed + size_delta) * 8).expect("This should be 64 at most") - 1);

        // TODO mask splitting; for now just | everything together, but this won't work in the
        // future
        let mask = size_bits.extract_from(rows.iter().fold(0x0u64, |acc, row| acc | row.mask));
        let mut jumplist_new = Jumplist::new(mask, size_delta);

        // partition by value
        let mut partitions: HashMap<u64, Vec<Row>> = HashMap::new();
        for row in rows.into_iter() {
            partitions.entry(size_bits.extract_from(row.value)).or_default().push(row);
        }

        // for each partition: create next byte jumplist if not all bytes have been consumed;
        // otherwise continue with next token or insert entry and finish (only in the first and
        // second case does indices contain more than one element)
        for (value, part_rows) in partitions.into_iter() {
            // partition into rows with trailing bytes and rows that terminate
            let (next_byte_rows, terminal_rows): (Vec<_>, Vec<_>) = part_rows.into_iter().partition(|row| row.token_size < bytes_consumed + size_delta);

            // create jumplist for all rows with trailing bytes and create entries for jumplist
            if !next_byte_rows.is_empty() {
                // create multiple entries for different sublists if at beginning
                let sublists = if bytes_consumed == 0 {
                    next_byte_rows.iter().map(|row| row.sublists.clone()).collect::<HashSet<Vec<_>>>().into_iter().collect_vec()
                } else {
                    vec![Vec::new()]
                };

                let next_entry_trailing = NextJumplistEntry::Jumplist(self.create_jumplist_bytes(token_index, next_byte_rows, bytes_consumed + size_delta)?);

                for sublist in sublists.into_iter() {
                    let entry = JumplistEntry::new(sublist, next_entry_trailing.clone());
                    jumplist_new.insert(value, entry);
                }
            };

            // create entries for all terminal rows (either a constructor if no tokens remain or a
            // jumplist for the next token); all matchers with a common prefix are contained in
            // this vector since they also must have the same value and therefore the same length
            let next_entry_terminal = if token_index + 1 < terminal_rows.first().expect("This can't be empty").matcher.token_list.list.len() {
                // tokens left => create jumplist for terminal matchers
                let matchers = terminal_rows.iter().map(|row| row.matcher).collect_vec();
                let jumplist = self.create_jumplist(token_index + 1, &matchers)?;
                NextJumplistEntry::Jumplist(jumplist)
            } else {
                // no tokens left => entry is row matcher itself
                assert_eq!(terminal_rows.len(), 1, "There must be exactly one row if no tokens follow");

                let matcher = terminal_rows.get(0).expect("There must be an element").matcher;
                let constructor = self.constructors.get(matcher.constructor.expect("The constructor must be set at this point")).ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing constructor") })?;
                NextJumplistEntry::Matcher(constructor.display.clone())
            };

            // same as above: create multiple entries for different sublists if at beginning
            let sublists = if bytes_consumed == 0 {
                terminal_rows.iter().map(|row| row.sublists.clone()).collect::<HashSet<Vec<_>>>().into_iter().collect_vec()
            } else {
                vec![Vec::new()]
            };

            for sublist in sublists.into_iter() {
                let entry = JumplistEntry::new(sublist, next_entry_terminal.clone());
                jumplist_new.insert(value, entry);
            }
        }

        let jumplist_new_id = self.jumplist_cache.insert(jumplist_new);
        Ok(jumplist_new_id)
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

/// Enumerates all values `v` s.t.:
/// - `v & mask_small == value`, and
/// - `v & mask_large = v`
///
/// This is necessary for creating jumplist indices for a jumplist with a single mask out of values
/// with potentially different, smaller masks. Returns [Option::None] in case the number of values
/// produced would exceed `threshold`.
fn expand_mask(value: u64, mask_small: u64, mask_large: u64, threshold: u64) -> Option<Vec<u64>> {
    trace_build!("value      ={:#064b}", value);
    trace_build!("mask_small ={:#064b}", mask_small);
    trace_build!("mask_large ={:#064b}", mask_large);

    if mask_small == mask_large {
        return Some(vec![value]);
    }

    let mask_delta = mask_large ^ mask_small;
    let shift = mask_delta.trailing_zeros();
    if (mask_delta >> shift) + 1 > threshold {
        return None;
    }

    // this could be done more efficiently but since we're dealing only with small values it should
    // be fine
    let values = (0..=(mask_delta >> shift)).map(|value| {
        (value << shift) & mask_delta
    }).collect::<HashSet<_>>();

    Some(values.into_iter().collect_vec())
}

/// Extracts a mask/value pair as well as subtables from a matcher conjunction.
fn extract_from_conjunction(conjunction: &HashSet<MatcherAtom>) -> (u64, u64, Vec<TableId>) {
    let mut mask = 0u64;
    let mut value = 0u64;
    let mut subtables = Vec::new();
    for atom in conjunction {
        match atom {
            MatcherAtom::Field { field, matcher } => {
                if let FieldMatcher::Value { mask: fmask, value: fvalue } = matcher {
                    mask |= *fmask;
                    value |= *fvalue;
                }
            },
            MatcherAtom::Submatchers { table, matchers: _ } => {
                subtables.push(*table);
            }
        }
    }

    (mask, value, subtables)
}
