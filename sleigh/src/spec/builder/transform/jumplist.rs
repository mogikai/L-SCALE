use petgraph::graph::NodeIndex;
use std::collections::{
    BTreeMap,
    BTreeSet,
    HashMap,
};
use std::fs::File;
use std::io::Write;

use crate::compiler::InternalCompilerResult;
use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    Submatchers,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::InternalError;
use crate::spec::ProcessorSpecificationError;
use crate::spec::jir::jumplist::{
    DataProvider,
    Jumplist,
    JumplistEntry,
    JumplistId,
    NextJumplistEntry,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::{
    ContextId,
    MOIRContext,
};
use crate::spec::moir::field::{
    FieldId,
    MOIRBitRange,
    MOIRField,
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
use itertools::Itertools;
use mogi_utilities::index_map::PrimaryMap;
use petgraph::Graph;
use petgraph::dot::Dot;
use tracing::error;

// Entries for different masks can be combined vertically by building up a hierarchy layer by layer
// or horizontally by ORing the mask bits together and filling missing entries by enumerating
// previously don't-care-bits. This is a tradeoff between path length and entry count.
// The value is the Hamming distance for which two masks will be joined together.
//
// Right now, this threshold only applies to a single value so multiple masks
// can produce different sets of values that together exceed this threshold.
// TODO find better way to do this.
// const MASK_EXPAND_THRESHOLD: u32 = 256u32.ilog2();
const MASK_EXPAND_THRESHOLD: u32 = 0u32;

/// Helpful information that is collected while building.
#[derive(Debug, Default)]
struct JumplistDiagnostics {
    // dummy jumplists have a mask of zero and only act as a branching node for other lists
    dummy_jumplist_count: usize,
    // number of lists created at the top level through `process_submatchers`
    root_level_count: usize,
    // number of lists created at the top level through `create_jumplist`
    byte_level_count: usize,

    // number of rows that are categorized as a child
    merged_row_count: usize,
    // number of rows that are categorized as a parent
    parent_row_count: usize,
    // number of total paths for mask matching
    unique_mask_paths: usize,

    // number of values propagated down from mask parents to their children
    propagated_parent_value_count: usize,
    // number of partitions by value and submatchers
    value_submatcher_partition_count: usize,
    // number of rows that have a next byte
    next_byte_row_count: usize,
    // number of rows that have a next token
    terminal_token_row_count: usize,
    // number of rows that have a next constructor
    terminal_matcher_row_count: usize,

    // total number of entries created
    entry_count: usize,
}

/// Create a [Jumplist] for the root [MOIRTable]
pub(super) struct JumplistBuilder<'a> {
    jumplist_cache: TypedIndexTable<JumplistId, Jumplist>,
    sublist_cache: HashMap<Submatchers, JumplistId>,
    tables: &'a TypedIndexTable<TableId, MOIRTable>,
    fields: &'a PrimaryMap<FieldId, MOIRField>,
    tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
    context: &'a TypedIndexTable<ContextId, MOIRContext>,
    varnodes: &'a PrimaryMap<VarNodeId, MOIRVarNode>,
    constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
    diagnostics: JumplistDiagnostics,
}

// Temporary data used for processing tokens and bytes

#[derive(Debug, Clone)]
struct Row<'a> {
    pub matcher: &'a Matcher,
    pub mask: u64,
    pub value: u64,
    pub data_provider: DataProvider,
    pub token_size: MOIRSize,
    pub submatchers_list: Vec<&'a Submatchers>,
}

impl<'a> Row<'a> {
    fn new(
        matcher: &'a Matcher,
        mask: u64,
        value: u64,
        data_provider: DataProvider,
        token_size: MOIRSize,
        submatchers_list: Vec<&'a Submatchers>,
    ) -> Self {
        Self {
            matcher,
            mask,
            value,
            data_provider,
            token_size,
            submatchers_list,
        }
    }
}

impl<'a> JumplistBuilder<'a> {
    pub(super) fn new(
        tables: &'a TypedIndexTable<TableId, MOIRTable>,
        fields: &'a PrimaryMap<FieldId, MOIRField>,
        tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
        context: &'a TypedIndexTable<ContextId, MOIRContext>,
        varnodes: &'a PrimaryMap<VarNodeId, MOIRVarNode>,
        constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
    ) -> Self {
        Self {
            jumplist_cache: TypedIndexTable::default(),
            sublist_cache: HashMap::new(),
            tables,
            fields,
            tokens,
            context,
            varnodes,
            constructors,
            diagnostics: JumplistDiagnostics::default(),
        }
    }

    pub(super) fn transform(
        mut self,
        root_table_id: TableId,
    ) -> InternalCompilerResult<TypedIndexTable<JumplistId, Jumplist>> {
        trace_build!("Processing {} tables into jumplists", self.tables.len());

        let root_table = self
            .tables
            .get(root_table_id)
            .ok_or_else(|| -> InternalError {
                todo!("Generate error report for missing root table");
            })?;
        let _root_matcher_ids = root_table.matchers.iter().map(|(key, _)| key).collect_vec();
        let submatchers = Submatchers::new(root_table_id, None);

        let root_jumplist_id = self.process_submatchers(&submatchers)?;

        trace_build!(
            "Created root jumplist ({} in total, {} cached sublists)",
            self.jumplist_cache.len(),
            self.sublist_cache.len(),
        );

        trace_build!("Jumplist diagnostics: {:#?}", self.diagnostics);

        // for (jumplist_id, jumplist) in self.jumplist_cache.iter() {
        //     if jumplist_id == root_jumplist_id {
        //         continue;
        //     }

        //     trace_build!("(jumplist_id, jumplist) = {:#x?}", (jumplist_id, jumplist));
        // }

        let _root_jumplist = self.jumplist_cache.get(root_jumplist_id).unwrap();
        trace_build!("(root_jumplist_id, root_jumplist) = {:#x?}", (root_jumplist_id, _root_jumplist));

        save_graph(root_jumplist_id, &self.jumplist_cache);

        Ok(self.jumplist_cache)
    }

    /// Process a table by creating and merging corresponding jumplists for each matcher in
    /// `matchers` in case the result hasn't been cached before. A separate list has to be created
    /// for each unique set of matchers.
    fn process_submatchers(
        &mut self,
        submatchers: &Submatchers,
    ) -> InternalCompilerResult<JumplistId> {
        match self.sublist_cache.get(submatchers) {
            Some(jumplist) => {
                // table was already processed => link to previous result
                Ok(*jumplist)
            }
            None => {
                // no cached result exists => create jumplist for matchers and store in sublist cache
                trace_build!("Processing submatchers {:?}", submatchers);

                let subtable = self
                    .tables
                    .get(submatchers.table)
                    .ok_or_else(|| -> InternalError {
                        todo!("Generate error report for missing table");
                    })?;

                let matchers = match &submatchers.matchers {
                    Some(submatcher_ids) => {
                        // restricted to concrete matchers (non-recursive) => use them
                        subtable
                            .matchers
                            .iter()
                            .filter_map(|(matcher_id, matcher)| {
                                if submatcher_ids.contains(&matcher_id) {
                                    Some(matcher)
                                } else {
                                    None
                                }
                            })
                            .collect_vec()
                    }
                    None => {
                        // shallow reference to table => find all possible matchers and use them
                        // TODO constrain here if necessary
                        subtable
                            .matchers
                            .iter()
                            .map(|(_, matcher)| matcher)
                            .collect_vec()
                    }
                };

                if matchers.is_empty() {
                    trace_build!("Table does not contain any matchers");
                    Err(ProcessorSpecificationError::UnresolvedSubtable)?
                }

                // dummy list to break infinite recursion
                let dummy_jumplist_id = self.jumplist_cache.insert(Jumplist::default());
                self.sublist_cache
                    .insert(submatchers.clone(), dummy_jumplist_id);

                // create single jumplist out of all matchers
                let new_jumplist = self.create_jumplist(0, &matchers)?;
                self.jumplist_cache.replace(dummy_jumplist_id, new_jumplist);

                self.diagnostics.root_level_count += 1;

                Ok(dummy_jumplist_id)
            }
        }
    }

    /// Create a single jumplist for multiple matchers. Multiple lists will be created and merged
    /// into one dummy list if the matchers have different [DataProvider]s at `token_index`.
    ///
    /// # Preconditions:
    /// - `matchers` must not be empty
    /// - each matcher must have a token list of length at least `token_index` + 1
    /// # Return on success
    /// - the newly created jumplist
    fn create_jumplist(
        &mut self,
        token_index: usize,
        matchers: &[&'a Matcher],
    ) -> InternalCompilerResult<Jumplist> {
        debug_assert_ne!(matchers.len(), 0, "There must be matchers");
        for matcher in matchers.iter() {
            if token_index >= matcher.token_list.list.len() {
                error!(
                    "Matcher token list not long enough: token_index={token_index} >= len={} \nMatcher: {matcher:?}",
                    matcher.token_list.list.len()
                );
            }
        }

        // this is just a stub precalculating everything necessary for `create_jumplist_bytes`

        // 1. extract everything necessary for token processing
        let rows = matchers
            .iter()
            .map(|matcher| {
                let (mask, value, data_provider, token_size, submatchers_list) = match matcher
                    .conjunctions
                    .get(token_index)
                    .zip(matcher.token_list.list.get(token_index))
                {
                    Some((conjunction, parent)) => {
                        let (mask, value, submatchers_list) = extract_from_conjunction(conjunction);
                        let (data_provider, token_size) = match parent {
                            Some(parent) => {
                                // Some => we know the size
                                let token_size = self.get_token_size(parent)?;
                                let data_provider = match parent {
                                    MOIRFieldParent::Token(_) => DataProvider::ByteStream,
                                    MOIRFieldParent::Context(cid) => DataProvider::Context(*cid),
                                };

                                (data_provider, token_size)
                            }
                            // None => we don't know the size (unconstrained subtable).
                            // More tokens can follow but since all tokens are matched
                            // before descending, this probably won't do what you would
                            // expect it to.
                            None => (DataProvider::ByteStream, 0),
                        };

                        (mask, value, data_provider, token_size, submatchers_list)
                    }
                    // This can be the case if subtables with more tokens than
                    // the matcher are used. The path terminates in this case.
                    None => (0, 0, DataProvider::ByteStream, 0, vec![]),
                };

                Ok(Row::new(matcher, mask, value, data_provider, token_size, submatchers_list))
            })
            .collect::<InternalCompilerResult<Vec<_>>>()?;

        // 2. partition rows by data provider
        let partitioned_rows = partition_by_data_provider(rows);

        // 3. create jumplists for each partition
        let jumplists = partitioned_rows
            .into_iter()
            .map(|(_, rows)| Ok(self.create_jumplist_bytes(rows, token_index, 0)?))
            .collect::<InternalCompilerResult<Vec<_>>>()?;

        self.diagnostics.byte_level_count += 1;

        // 4. either use directly or combine
        if jumplists.len() > 1 {
            // multiple lists => create dummy list
            self.diagnostics.dummy_jumplist_count += 1;

            let mut jumplist_new = Jumplist::default();
            for jumplist in jumplists.into_iter() {
                let jumplist_id = self.jumplist_cache.insert(jumplist);
                jumplist_new.insert(
                    0,
                    JumplistEntry::new(vec![], NextJumplistEntry::TrailingSame(jumplist_id)),
                    false,
                )?;
                self.diagnostics.entry_count += 1;
            }

            Ok(jumplist_new)
        } else {
            // only a single list => use directly
            Ok(jumplists
                .into_iter()
                .nth(0)
                .expect("At least one list must have been created"))
        }
    }

    /// Recursion used to process tokens of different length but with the same provider into a
    /// single jumplist by repeatedly splitting larger tokens into the least common size and then
    /// building jumplists on top of that.
    ///
    /// # Preconditions
    /// - `rows` must not be empty
    /// - `data_provider` must be the same for all rows
    /// # Return on success
    /// - the newly created jumplist
    fn create_jumplist_bytes(
        &mut self,
        mut rows: Vec<Row<'a>>,
        token_index: usize,
        bytes_consumed: MOIRSize,
    ) -> InternalCompilerResult<Jumplist> {
        debug_assert_ne!(rows.len(), 0, "There must be rows");
        for row in &rows {
            debug_assert_eq!(
                row.data_provider,
                rows.get(0).expect("We just checked this").data_provider,
                "Data providers must be equal"
            );
        }

        // new size is equal to size of smallest row
        let size_delta = rows
            .iter()
            .map(|row| row.token_size)
            .min()
            .expect("We checked for this")
            - bytes_consumed;

        let size_bits = MOIRBitRange::new(
            MOIRSize::try_from(bytes_consumed * 8).unwrap(),
            MOIRSize::try_from((bytes_consumed + size_delta) * 8).unwrap(),
        );

        // We create intermediary jumplists to handle different masks without creating additional
        // entries (TODO find balance between more entries and deeper hierarchies). This is done by
        // descending a tree defined by subset relations of masks while building a jumplist for
        // each tree node. Since there has to be a root, we start with the combination of all masks
        // and then descent into smaller and smaller masks.

        // sort by mask so submask relations are satisfied which allows us to repeatedly
        // iterate over and remove rows
        rows.sort_by_key(|row| row.mask);

        let jumplists = self.create_jumplist_mask(rows, token_index, bytes_consumed, size_bits)?;

        // combine if multiple, use directly if only one
        let jumplist = if jumplists.len() > 1 {
            let mut jumplist_new = Jumplist::default();
            for jumplist in jumplists.into_iter() {
                let jumplist_id = self.jumplist_cache.insert(jumplist);
                let entry = JumplistEntry::new(vec![], NextJumplistEntry::TrailingSame(jumplist_id));
                jumplist_new.insert(0, entry, false)?;
                self.diagnostics.entry_count += 1;
            }
            jumplist_new
        } else {
            jumplists
                .into_iter()
                .nth(0)
                .expect("There must be at least one jumplist")
        };

        Ok(jumplist)
    }

    /// Recursion used to process rows with different masks into a single jumplist by repeatedly
    /// creating jumplists for smaller masks and arranging them into a tree. Leaf jumplists will be
    /// collected in `leaf_lists` for insertion into the final list.
    ///
    /// # Preconditions
    /// - `data_provider` must be the same for all rows
    /// # Return on success
    /// - (potentially multiple) jumplists, one for each distinct matching path
    fn create_jumplist_mask(
        &mut self,
        rows: Vec<Row<'a>>,
        token_index: usize,
        bytes_consumed: MOIRSize,
        size_bits: MOIRBitRange,
    ) -> InternalCompilerResult<Vec<Jumplist>> {
        if rows.is_empty() {
            // base case: nothing to create
            return Ok(vec![]);
        }
        for row in &rows {
            debug_assert_eq!(
                row.data_provider,
                rows.get(0).expect("We just checked this").data_provider,
                "Data providers must be equal"
            );
        }

        let merged_rows = merge_rows(rows, size_bits);
        self.diagnostics.merged_row_count += merged_rows.len();

        let hierarchy = arrange_into_hierarchy(merged_rows);
        self.diagnostics.unique_mask_paths += hierarchy.len();

        // we still need to sort the hierarchy by mask so higher masks have higher priority

        // collect child jumplists and then either use directly if only one is created
        // or combine using a dummy list
        let mut child_jumplists = Vec::new();
        for (set_mask, set_rows_child, set_rows_parent) in hierarchy {
            // process parent set into jumplists
            let jumplists_parents =
                self.create_jumplist_mask(set_rows_parent, token_index, bytes_consumed, size_bits)?;

            // process child set into jumplist
            let size_delta = size_bits.bytes().expect("This must be a multiple of 8");
            let mut jumplist_child = Jumplist::new(
                set_mask,
                size_delta,
                set_rows_child
                    .get(0)
                    .expect("We just checked this")
                    .data_provider,
            );

            // propagate parent lists
            for jumplist_parent in jumplists_parents.into_iter() {
                let jumplist_parent_id = self.jumplist_cache.insert(jumplist_parent);
                let jumplist_parent = self
                    .jumplist_cache
                    .get(jumplist_parent_id)
                    .expect("We just inserted this");
                for propagated_value in jumplist_parent
                    .values()
                    .map(|parent_value| parent_value & set_mask)
                    .unique()
                {
                    // use NextJumplistEntry::TrailingSame since we want to continue but not consume any
                    // bytes
                    let propagated_entry =
                        JumplistEntry::new(vec![], NextJumplistEntry::TrailingSame(jumplist_parent_id));
                    jumplist_child.insert(propagated_value, propagated_entry, false)?;
                    self.diagnostics.entry_count += 1;
                    self.diagnostics.propagated_parent_value_count += 1;
                }
            }

            // partition by value and submatchers
            let mut partitions: BTreeMap<(u64, Vec<&'a Submatchers>), Vec<Row>> = BTreeMap::new();
            for row in set_rows_child.into_iter() {
                let submatchers_list = if bytes_consumed == 0 {
                    // first iteration: take sublists into account
                    row.submatchers_list.clone() // TODO how to do this without cloning?
                } else {
                    vec![]
                };

                partitions
                    .entry((size_bits.extract(row.value), submatchers_list))
                    .or_default()
                    .push(row);
            }

            self.diagnostics.value_submatcher_partition_count += partitions.len();

            // for each partition: create next byte jumplist if not all bytes have been consumed;
            // otherwise continue with next token or matcher
            for ((value, submatchers_list), part_rows) in partitions.into_iter() {
                // process submatchers into sublist
                let sublist_ids = submatchers_list
                    .into_iter()
                    .map(|submatchers| self.process_submatchers(submatchers))
                    .collect::<InternalCompilerResult<Vec<_>>>()?;

                // partition into rows with trailing bytes and rows that terminate
                let (next_byte_rows, terminal_rows): (Vec<_>, Vec<_>) = part_rows
                    .into_iter()
                    .partition(|row| row.token_size > bytes_consumed + size_delta);

                // partition rows that terminate into rows that terminate in a matcher or in more
                // tokens (the row might end despite remaining tokens since these might belong to a
                // subtable => if an empty conjunction is encountered this is also treated as an end)
                let (terminal_token_rows, terminal_matcher_rows): (Vec<_>, Vec<_>) =
                    terminal_rows.into_iter().partition(|row| {
                        row.matcher
                            .conjunctions
                            .get(token_index + 1)
                            .map(|conjunction| !conjunction.is_empty())
                            .unwrap_or(false)
                    });

                self.diagnostics.next_byte_row_count += next_byte_rows.len();
                self.diagnostics.terminal_token_row_count += terminal_token_rows.len();
                self.diagnostics.terminal_matcher_row_count += terminal_matcher_rows.len();

                // create entries for jumplist of trailing bytes
                if !next_byte_rows.is_empty() {
                    let expanded_values = expand_rows(&next_byte_rows, set_mask, value, size_bits);

                    let trailing_jumplist =
                        self.create_jumplist_bytes(next_byte_rows, token_index, bytes_consumed + size_delta)?;

                    // nothing to optimize here
                    let trailing_jumplist_id = self.jumplist_cache.insert(trailing_jumplist);
                    let next_entry_trailing = NextJumplistEntry::TrailingNext(trailing_jumplist_id);

                    let entry = JumplistEntry::new(sublist_ids.clone(), next_entry_trailing);
                    // first entry for non-expanded values...
                    jumplist_child.insert(value, entry.clone(), false)?;
                    self.diagnostics.entry_count += 1;

                    // ...then for expanded values
                    for expanded_value in expanded_values.into_iter() {
                        jumplist_child.insert(expanded_value, entry.clone(), false)?;
                        self.diagnostics.entry_count += 1;
                    }
                };

                // create entries for terminal rows that still have tokens remaining
                if !terminal_token_rows.is_empty() {
                    let expanded_values = expand_rows(&terminal_token_rows, set_mask, value, size_bits);

                    let terminal_token_matchers = terminal_token_rows
                        .into_iter()
                        .map(|row| row.matcher)
                        .collect_vec();

                    // nothing to optimize here
                    let next_token_jumplist = self.create_jumplist(token_index + 1, &terminal_token_matchers)?;
                    let next_token_jumplist_id = self.jumplist_cache.insert(next_token_jumplist);

                    let next_entry_tokens = NextJumplistEntry::TrailingNext(next_token_jumplist_id);
                    let entry = JumplistEntry::new(sublist_ids.clone(), next_entry_tokens);
                    jumplist_child.insert(value, entry.clone(), false)?;
                    self.diagnostics.entry_count += 1;

                    for expanded_value in expanded_values.into_iter() {
                        jumplist_child.insert(expanded_value, entry.clone(), false)?;
                        self.diagnostics.entry_count += 1;
                    }
                }

                // create entries for terminal row that ends in a matcher
                if let Some(terminal_matcher_row) = terminal_matcher_rows.first() {
                    let expanded_values = expand_rows(&terminal_matcher_rows, set_mask, value, size_bits);

                    // this assertion does not have to hold true since two matchers can have the
                    // same primary matching path but different subtables (TODO should we check
                    // this?)
                    // assert!(terminal_matcher_rows.len() < 2, "There can't be more than one matcher in the same path");
                    let next_entry_matcher = NextJumplistEntry::Constructor {
                        tid: terminal_matcher_row.matcher.table_id,
                        cid: terminal_matcher_row.matcher.constructor_id,
                    };

                    let entry = JumplistEntry::new(sublist_ids, next_entry_matcher);
                    jumplist_child.insert(value, entry.clone(), false)?;
                    self.diagnostics.entry_count += 1;

                    for expanded_value in expanded_values.into_iter() {
                        jumplist_child.insert(expanded_value, entry.clone(), false)?;
                        self.diagnostics.entry_count += 1;
                    }
                }
            }

            child_jumplists.push(jumplist_child);
        }

        Ok(child_jumplists)
    }

    fn get_token_size(
        &self,
        parent: &MOIRFieldParent,
    ) -> InternalCompilerResult<MOIRSize> {
        match parent {
            MOIRFieldParent::Token(tid) => self
                .tokens
                .get(*tid)
                .ok_or_else(|| -> InternalError {
                    todo!("Generate error report for missing token");
                })
                .map(|token| token.bytes),
            MOIRFieldParent::Context(cid) => {
                let vnid = self
                    .context
                    .get(*cid)
                    .ok_or_else(|| -> InternalError {
                        todo!("Generate error report for missing context");
                    })?
                    .varnode;
                let varnode = self.varnodes.get(vnid).ok_or_else(|| -> InternalError {
                    todo!("Generate error report for missing varnode");
                })?;
                Ok(varnode.size)
            }
        }
    }
}

/// Enumerates all values `v` s.t.:
/// - `v & mask_small == value`, and
/// - `v & mask_large == v`
/// - `v != value`
///
/// This is necessary for creating jumplist indices for a jumplist with a single mask out of values
/// with potentially different, smaller masks.
fn expand_mask(
    value: u64,
    mask_small: u64,
    mask_large: u64,
) -> Vec<u64> {
    /* value        = 0b0000
     * mask_small   = 0b0011
     * mask_large   = 0b1111
     * values[0]    = 0b0000
     * values[1]    = 0b0100
     * values[2]    = 0b1000
     * values[3]    = 0b1100
     */

    let mask_delta = mask_large ^ mask_small;
    let num_values = 2u64.pow(mask_delta.count_ones()) - 1;
    let mut new_values = Vec::with_capacity(usize::try_from(num_values).unwrap());

    // (position, value) tuples
    let mut delta_positions = (0..u64::BITS)
        .filter(|&position| (mask_delta >> position) & 1 == 1)
        .map(|position| (position, 0))
        .collect_vec();

    for _ in 0..=num_values {
        // add new value to values depending on the current delta_positions
        let mut new_value = value;
        for &(dpi, dpv) in delta_positions.iter() {
            new_value |= dpv << dpi;
        }

        if new_value != value {
            new_values.push(new_value);
        }

        // increase delta_positions by one
        for (_, dpv) in delta_positions.iter_mut() {
            if *dpv == 0 {
                *dpv = 1;
                break;
            } else {
                *dpv = 0;
            }
        }
    }

    new_values
}

/// Calculate the largest common mask of all rows and expand values.
fn expand_rows(
    rows: &Vec<Row>,
    mask: u64,
    value: u64,
    size_bits: MOIRBitRange,
) -> Vec<u64> {
    let largest_common_mask = rows
        .iter()
        .fold(!0u64, |acc, row| acc & size_bits.extract(row.mask));

    expand_mask(value, largest_common_mask, mask)
}

/// Extracts a mask/value pair as well as subtables from a matcher conjunction.
fn extract_from_conjunction(conjunction: &BTreeSet<MatcherAtom>) -> (u64, u64, Vec<&Submatchers>) {
    let mut mask = 0u64;
    let mut value = 0u64;
    let mut subtables = Vec::new();
    for atom in conjunction {
        match atom {
            MatcherAtom::Field { field_id: _, matcher } => {
                if let FieldMatcher::Value {
                    mask: fmask,
                    value: fvalue,
                } = matcher
                {
                    mask |= *fmask;
                    // TODO handle endianness when extracting value
                    value |= *fvalue;
                }
            }
            MatcherAtom::Submatchers(submatchers) => {
                subtables.push(submatchers);
            }
        }
    }

    (mask, value, subtables)
}

/// Returns true if `submask` is a submask of `supermask`,
/// i.e. if all bits set in `submask` are also set in `supermask`
fn is_submask(
    submask: u64,
    supermask: u64,
) -> bool {
    (submask ^ supermask) & submask == 0
}

/// Rows with different data providers can't be merged together so we need to
/// partition them before passing them to [create_jumplist].
fn partition_by_data_provider(rows: Vec<Row>) -> BTreeMap<DataProvider, Vec<Row>> {
    let mut partitions = BTreeMap::<DataProvider, Vec<Row>>::new();
    for row in rows.into_iter() {
        partitions.entry(row.data_provider).or_default().push(row)
    }

    partitions
}

/// Merge rows into sets such that the set mask (the disjunction of all masks of this set)
/// has a Hamming distance less than or equal than MASK_EXPAND_THRESHOLD from every mask in
/// this set.
fn merge_rows<'a>(
    rows: Vec<Row<'a>>,
    size_bits: MOIRBitRange,
) -> Vec<(u64, Vec<Row<'a>>)> {
    let mut sets = Vec::<(u64, Vec<Row<'a>>)>::new();
    'next_row: for row in rows.into_iter() {
        let row_mask = size_bits.extract(row.mask);

        'next_set: for (set_mask, set_rows) in sets.iter_mut() {
            let new_mask = *set_mask | row_mask;

            for set_row in &*set_rows {
                if (size_bits.extract(set_row.mask) ^ new_mask).count_ones() > MASK_EXPAND_THRESHOLD {
                    // new row does not fit into this set => check next set
                    continue 'next_set;
                }
            }

            // new row fits into this set => insert and continue with next row
            set_rows.push(row);
            *set_mask = new_mask;
            continue 'next_row;
        }

        // new row does not fit into any set => create new one
        sets.push((row_mask, vec![row]));
    }

    sets
}

/// Arrange all merged rows into a hierarchy w.r.t. the supermask relation.
fn arrange_into_hierarchy<'a>(merged_rows: Vec<(u64, Vec<Row<'a>>)>) -> Vec<(u64, Vec<Row<'a>>, Vec<Row<'a>>)> {
    let mut sets = Vec::<(u64, Vec<Row<'a>>, Vec<Row<'a>>)>::new();

    'next_merged: for (merged_mask, mut merged_rows) in merged_rows.into_iter() {
        // set_children is not modified since all rows with equal mask were already merged in the
        // previous step
        for (set_mask, _, set_parents) in sets.iter_mut() {
            if is_submask(*set_mask, merged_mask) {
                // set mask is submask of merged mask => merged rows are parents
                set_parents.append(&mut merged_rows);
                continue 'next_merged;
            }
        }

        // set mask is not a submask of any set => add new set with itself as the child
        sets.push((merged_mask, merged_rows, vec![]));
    }

    sets
}

fn save_graph(
    root: JumplistId,
    jumplist_cache: &TypedIndexTable<JumplistId, Jumplist>,
) {
    // depth-first traversal
    let mut stack = vec![root];
    let mut nodes = HashMap::<NextJumplistEntry, NodeIndex>::new();
    let mut graph = Graph::<String, String>::new();

    while let Some(top) = stack.pop() {
        let list = jumplist_cache.get(top).unwrap();
        let list_node = *nodes
            .entry(NextJumplistEntry::TrailingNext(top))
            .or_insert_with(|| graph.add_node(format!("L{}", usize::from(top))));

        for (value, entries) in list.entries() {
            for entry in entries {
                // process next_entry
                let entry_node = *nodes.entry(entry.next_entry.clone()).or_insert_with(|| {
                    // not seen => insert node and push onto stack
                    let entry_node_weight = match &entry.next_entry {
                        NextJumplistEntry::TrailingNext(jlid) => {
                            stack.push(*jlid);
                            format!("L{}", usize::from(*jlid))
                        }
                        NextJumplistEntry::TrailingSame(jlid) => {
                            stack.push(*jlid);
                            format!("S{}", usize::from(*jlid))
                        }
                        NextJumplistEntry::Constructor { cid, tid: _ } => format!("C{}", cid),
                    };

                    graph.add_node(entry_node_weight)
                });

                graph.add_edge(list_node, entry_node, format!("NE{:#x}", value));

                // process sublists
                for sublist in &entry.sublists {
                    let sublist_node = *nodes
                        .entry(NextJumplistEntry::TrailingNext(*sublist))
                        .or_insert_with(|| {
                            // not seen => insert node and push onto stack
                            let sublist_node_weight = format!("L{}", usize::from(*sublist));
                            stack.push(*sublist);
                            graph.add_node(sublist_node_weight)
                        });

                    graph.add_edge(list_node, sublist_node, format!("SL{:#x}", value));
                }
            }
        }
    }

    let dot = Dot::with_config(&graph, &[]);
    let mut file = File::create("jumplist_graph.gv").unwrap();
    writeln!(file, "{:?}", dot).unwrap();
}
