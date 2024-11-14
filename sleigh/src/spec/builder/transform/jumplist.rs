use std::collections::hash_map::Entry;
use std::collections::{
    HashMap,
    HashSet,
};

use chumsky::container::{
    Container,
    Seq,
};
use itertools::Itertools;
use num::PrimInt;

use crate::compiler::ast::pcode::deref;
use crate::compiler::build::constructor::matcher::{
    FieldMatcher, Matcher, MatcherAtom, MatcherId, Submatchers
};
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::spec::jir::jumplist::{
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
    MOIRContext,
    MOIRContextId,
};
use crate::spec::moir::field::{
    MOIRBitRange,
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

/// Create a [Jumplist] for the root [MOIRTable]
pub(super) struct JumplistBuilder<'a> {
    jumplist_cache: TypedIndexTable<JumplistId, Jumplist>,
    sublist_cache: HashMap<Submatchers, JumplistId>,
    tables: &'a TypedIndexTable<TableId, MOIRTable>,
    tokens: &'a TypedIndexTable<TokenId, MOIRToken>,
    context: &'a TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &'a TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &'a TypedIndexTable<ConstructorId, MOIRConstructor>,
}

// Temporary data used for processing tokens and bytes

#[derive(Debug, Clone)]
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

impl<'a> JumplistBuilder<'a> {
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

    pub(super) fn transform(
        mut self,
        root_table_id: TableId,
    ) -> SleighCompilerResult<(JumplistId, TypedIndexTable<JumplistId, Jumplist>)> {
        trace_build!("Processing {} tables into jumplists", self.tables.len());

        let root_table = self
            .tables
            .get(root_table_id)
            .ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing root table");
            })?;
        let root_matcher_ids = root_table.matchers.iter().map(|(key, _)| key).collect_vec();
        let submatchers = Submatchers::new(root_table_id, root_matcher_ids);

        let root_jumplist_id = self.process_submatchers(&submatchers)?;

        eprintln!(
            "Created root jumplist ({} in total, {} cached sublists)",
            self.jumplist_cache.len(),
            self.sublist_cache.len(),
        );

        // self.jumplist_cache
        //     .iter()
        //     .filter(|(id, _)| *id != root_jumplist_id)
        //     .for_each(|jumplist| {
        //         eprintln!("jumplist = {:#?}", jumplist);
        //     });

        // let root_jumplist = self
        //     .jumplist_cache
        //     .get(root_jumplist_id)
        //     .expect("We just created this");

        // eprintln!("root_jumplist = {:#?}", root_jumplist);

        Ok((root_jumplist_id, self.jumplist_cache))
    }

    /// Process a table by creating and merging corresponding jumplists for each matcher in
    /// `matchers` in case the result hasn't been cached before. A separate list has to be created
    /// for each unique set of matchers.
    fn process_submatchers(
        &mut self,
        submatchers: &Submatchers,
    ) -> SleighCompilerResult<JumplistId> {
        match self.sublist_cache.get(submatchers) {
            Some(jumplist) => {
                // table was already processed => link to previous result
                Ok(*jumplist)
            }
            None => {
                // no cached result exists => create jumplist for matchers and store in sublist cache (we don't use the submatchers
                // field because tokens can be different as long as their sizes match)
                trace_build!("Processing submatchers {:?}", submatchers);

                let subtable = self
                    .tables
                    .get(submatchers.table)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing table");
                    })?;

                let matchers = subtable.matchers
                    .iter()
                    .filter(|(matcher_id, matcher)| submatchers.matchers.contains(matcher_id))
                    .map(|(_, matcher)| matcher)
                    .collect_vec();

                // create single jumplist out of all matchers
                let jumplist = self.create_jumplist(0, &matchers)?;

                // cache and return
                self.sublist_cache.insert(submatchers.clone(), jumplist);
                Ok(jumplist)
            }
        }
    }

    /// Create a single jumplist for (potentially) multiple matchers.
    ///
    /// # Preconditions:
    /// - `matchers` must not be empty
    /// - each matcher must have a token list of length at least `token_index` + 1
    fn create_jumplist(
        &mut self,
        token_index: usize,
        matchers: &[&Matcher],
    ) -> SleighCompilerResult<JumplistId> {
        // this is just a stub precalculating everything necessary for `create_jumplist_bytes`

        // 1. extract everything necessary for token processing
        let rows = matchers
            .iter()
            .map(|matcher| {
                let (mask, value, token_size, sublists) = match matcher.conjunctions.get(token_index).zip(matcher.token_list.list.get(token_index)) {
                    Some((conjunction, parent)) => {
                        let (mask, value, submatchers_list) = extract_from_conjunction(conjunction);
                        let token_size = self.get_token_size(parent)?;

                        let sublists = submatchers_list
                            .into_iter()
                            .map(|submatchers| self.process_submatchers(submatchers))
                            .collect::<SleighCompilerResult<Vec<_>>>()?;

                        (mask, value, token_size, sublists)
                    },
                    None => {
                        (0, 0, 0, vec![])
                    }
                };

                Ok(Row::new(matcher, mask, value, token_size, sublists))
            })
        .collect::<SleighCompilerResult<Vec<_>>>()?;

        // 2. pass to recursive function
        self.create_jumplist_bytes(token_index, rows, 0)
    }

    /// Recursion used to process tokens of different length into a single
    /// jumplist by repeatedly splitting larger tokens into the least common size and then
    /// building jumplists on top of that.
    ///
    /// # Preconditions
    /// - `rows` must not be empty
    /// # Return on success
    ///  - the new jumplist id
    fn create_jumplist_bytes(
        &mut self,
        token_index: usize,
        rows: Vec<Row>,
        bytes_consumed: MOIRSize,
    ) -> SleighCompilerResult<JumplistId> {
        assert_ne!(rows.len(), 0, "There must be rows");

        /// new size is equal to size of smallest row (max. 1 for now so we can always to mask
        /// splitting)
        let size_delta = rows
            .iter()
            .map(|row| row.token_size)
            .min()
            .expect("We checked for this") - bytes_consumed;

        // TODO remove this
        let size_delta = size_delta.min(1);

        let size_bits = MOIRBitRange::new(
            u8::try_from(bytes_consumed * 8).expect("This should be 64 at most"),
            u8::try_from((bytes_consumed + size_delta) * 8).expect("This should be 64 at most"),
        );

        // TODO mask splitting; for now just | everything together and expand subsets, but this
        // won't work in the future
        let mask_combined = size_bits.extract_from(rows.iter().fold(0x0u64, |acc, row| acc | row.mask));
        let mut jumplist_new = Jumplist::new(mask_combined, size_delta);

        // partition by value and sublists
        let mut partitions: HashMap<(u64, Vec<JumplistId>), Vec<Row>> = HashMap::new();
        for row in rows.into_iter() {
            let sublists = if bytes_consumed == 0 {
                row.sublists.clone() // TODO how to do this without cloning?
            } else {
                vec![]
            };

            // here we do mask expansion to create necessary additional entries
            // for rows with a mask smaller than `mask_combined`; since we restrict
            // ourselves to one byte for now, this can always be done with a low
            // overhead
            let row_value = size_bits.extract_from(row.value);
            let row_mask = size_bits.extract_from(row.mask);
            let row_value_expanded = expand_mask(row_value, row_mask, mask_combined, 256).expect("We restrict ourselves to one byte so this always succeeds");

            for value in row_value_expanded.into_iter() {
                partitions
                    .entry((value, sublists.clone()))
                    .or_default()
                    .push(row.clone());
            }
        }

        // sort partitions themselves by mask since they might have the same value
        // and therefore the same entry, but might differ in priority

        // for each partition: create next byte jumplist if not all bytes have been consumed;
        // otherwise continue with next token or matcher
        for ((value, sublists), mut part_rows) in partitions.into_iter() {
            // sort part_rows by mask so rows with a higher mask have higher precedence
            part_rows.sort_by(|lhs, rhs| size_bits.extract_from(lhs.mask).cmp(&size_bits.extract_from(rhs.mask)).reverse());
            eprintln!("part_rows.len() = {:#?}", part_rows.len());

            // partition into rows with trailing bytes and rows that terminate
            let (next_byte_rows, terminal_rows): (Vec<_>, Vec<_>) = part_rows
                                                  .into_iter()
                                                  .partition(|row| row.token_size > bytes_consumed + size_delta);

            // partition rows that terminate into rows that terminate in a matcher or in more
            // tokens (the row might end despite remaining tokens since these might belong to a
            // subtable => if an empty conjunction is encountered this is also treated as an end)
            let (terminal_token_rows, terminal_matcher_rows): (Vec<_>, Vec<_>) = terminal_rows.into_iter().partition(|row| row.matcher.conjunctions.get(token_index + 1).map(|conjunction| !conjunction.is_empty()).unwrap_or(false));

            // create entries for jumplist of trailing bytes
            if !next_byte_rows.is_empty() {
                let next_entry_trailing = NextJumplistEntry::Jumplist(self.create_jumplist_bytes(
                        token_index,
                        next_byte_rows,
                        bytes_consumed + size_delta,
                )?);

                let entry = JumplistEntry::new(sublists.clone(), next_entry_trailing.clone());
                jumplist_new.insert(value, entry.clone())?;
            };

            // create entries for terminal rows that still have tokens remaining
            if !terminal_token_rows.is_empty() {
                let terminal_token_matchers = terminal_token_rows.into_iter().map(|row| row.matcher).collect_vec();
                let next_entry_tokens = NextJumplistEntry::Jumplist(self.create_jumplist(token_index + 1, &terminal_token_matchers)?);
                let entry = JumplistEntry::new(sublists.clone(), next_entry_tokens.clone());
                jumplist_new.insert(value, entry.clone())?;
            }

            // create entries for terminal row that ends in a matcher
            if let Some(terminal_matcher_row) = terminal_matcher_rows.first() {
                // since we don't remove duplicates when expanding values, multiple matchers on the
                // same path might be possible; this is no problem for matching though since
                // the higher precendence ones always come before so we can ignore the others
                // assert!(terminal_matcher_rows.len() < 2, "There can't be more than one matcher in the same path");
                let next_entry_matcher = NextJumplistEntry::Matcher(terminal_matcher_row.matcher.display.as_ref().expect("This must be set at this point").clone());

                let entry = JumplistEntry::new(sublists, next_entry_matcher.clone());
                jumplist_new.insert(value, entry.clone())?;
            }
        }

        let jumplist_new_id = self.jumplist_cache.insert(jumplist_new);
        Ok(jumplist_new_id)
    }

    /// Recursion used to process rows with a given length and different masks
    /// into a single jumplist by 
    ///
    /// # Preconditions
    /// - `rows` must not be empty
    /// # Return on success
    ///  - the new jumplist id
    fn create_jumplist_masks(partitions: Vec<(u64, Row)>) -> SleighCompilerResult<JumplistId> {

    }

    fn get_token_size(
        &self,
        parent: &MOIRFieldParent,
    ) -> SleighCompilerResult<MOIRSize> {
        match parent {
            MOIRFieldParent::Token(tid) => self
                .tokens
                .get(*tid)
                .ok_or_else(|| -> SleighCompilerError {
                    todo!("Generate error report for missing token");
                })
            .map(|token| token.bytes),
            MOIRFieldParent::Context(cid) => {
                let vnid = self
                    .context
                    .get(*cid)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing context");
                    })?
                .varnode;
                let varnode = self
                    .varnodes
                    .get(vnid)
                    .ok_or_else(|| -> SleighCompilerError {
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
///
/// This is necessary for creating jumplist indices for a jumplist with a single mask out of values
/// with potentially different, smaller masks. Returns [Option::None] in case the number of values
/// produced would exceed `threshold`.
fn expand_mask(
    value: u64,
    mask_small: u64,
    mask_large: u64,
    threshold: u64,
) -> Option<Vec<u64>> {
    if mask_small == mask_large {
        return Some(vec![value]);
    }

    /* value        = 0b0000
     * mask_small   = 0b0011
     * mask_large   = 0b1111
     * values[0]    = 0b0000
     * values[1]    = 0b0100
     * values[2]    = 0b1000
     * values[3]    = 0b1100
     */

    let mask_delta = mask_large ^ mask_small;
    let num_values = 2u64.pow(mask_delta.count_ones());
    if num_values > threshold {
        return None;
    }

    let mut new_values = Vec::with_capacity(usize::try_from(num_values).unwrap());
    new_values.push(value);

    // (position, value) tuples
    let mut delta_positions = (0..u64::BITS).filter(|&position| (mask_delta >> position) & 1 == 1).map(|position| (position, 0)).collect_vec();
    for _ in 0..num_values {
        // add new value to values depending on the current delta_positions
        let mut new_value = value;
        for &(dpi, dpv) in delta_positions.iter() {
            new_value |= dpv << dpi;
        }
        new_values.push(new_value);

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

    Some(new_values)
}

/// Extracts a mask/value pair as well as subtables from a matcher conjunction.
fn extract_from_conjunction(conjunction: &HashSet<MatcherAtom>) -> (u64, u64, Vec<&Submatchers>) {
    let mut mask = 0u64;
    let mut value = 0u64;
    let mut subtables = Vec::new();
    for atom in conjunction {
        match atom {
            MatcherAtom::Field { field, matcher } => {
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
