use core::panic;
use std::collections::{
    BTreeSet,
    HashMap,
    HashSet,
};

use chumsky::container::Container;
use itertools::Itertools;
use num::PrimInt;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    MatcherReference,
    TokenList,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::moir::token::TokenId;
use crate::spec::ProcessorSpecificationError;

impl SleighCompiler {
    /// Create a [Matcher] from a [SASTBitPatternConstraint]. Multiple matchers might be created
    /// due to the splitting of Or operations and the integration of subtables. After the creation,
    /// the following properties hold:
    ///
    /// - Each matcher contains a fixed-length, known list of tokens
    ///     - Each token list has unique context tokens at the beginning
    /// - Each matcher consists only of And statement which are trivial (mask-value)
    pub(super) fn create_matchers(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let matchers = self.create_matchers_recursive(constraint)?;

        Ok(matchers)
    }

    // private helper functions

    /// Recursive entry called through [Self::create_matchers]
    fn create_matchers_recursive(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let (constraint, _span) = constraint.into_tuple();
        let new_matchers = match constraint {
            SASTBitPatternConstraint::Ident(ident) => self.process_ident(ident),
            SASTBitPatternConstraint::BinaryOp(lhs, op, rhs) => self.process_binary_op(lhs, op, rhs),
            SASTBitPatternConstraint::UnaryOp(op, operand) => self.process_unary_op(op, operand),
            SASTBitPatternConstraint::AdvancedConstraint(lhs, op, pattern) => {
                self.process_advanced_constraint(lhs, op, pattern)
            }
            SASTBitPatternConstraint::Invalid => {
                todo!("Generate error report for an invalid constraint");
            }
        };

        new_matchers
    }

    /// Process an identifier into potentially multiple matchers by creating
    /// [Matcher]s for the tokens and symbols referenced.
    fn process_ident(
        &self,
        ident: Spanned<SASTIdent>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        // Ident case: freestanding fields, varnodes or subtables linking global to local symbols. This
        // is one of the base cases and yields multiple simple [Matcher] with [MatcherAtom::Linkage]
        // and the corresponding symbols, fields and tokens.

        let symbol = self.spec_builder.get_symbol(&ident.clone().transmute())?;

        // we don't want empty token lists so we'll use the zero token whenever there isn't an
        // actual underlying token
        let zero_token = self
            .spec_builder
            .zero_token()
            .ok_or_else(|| -> SleighCompilerError {
                todo!("The zero token has to be defined at this point.");
            })?;

        let matchers = match symbol {
            MOIRSymbolId::Table(tid) => {
                // expansion strategy: create one matcher per unique token list of matchers
                // contained in the subtable
                let table = self
                    .spec_builder
                    .table_by_symbol_id(*tid)
                    .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing symbol") })?;

                // partition table matchers by token list
                let mut partition: HashMap<_, Vec<_>> = HashMap::new();
                table.matchers.iter().for_each(|(id, matcher)| {
                    if let Some(list) = partition.get_mut(&matcher.token_list.list) {
                        list.push(id);
                    } else {
                        partition.insert(matcher.token_list.list.clone(), vec![id]);
                    }
                });

                // create a new matcher for each bin
                partition
                    .into_iter()
                    .map(|(token_list, matchers)| {
                        // associate all matchers with equal token_list with a new matcher
                        let token_list = if token_list.is_empty() {
                            vec![MOIRFieldParent::Token(zero_token)]
                        } else {
                            token_list
                        };

                        let atom = MatcherAtom::Submatchers { table: *tid, matchers };
                        let mut conjunctions = vec![HashSet::from([atom])];
                        conjunctions.append(&mut vec![HashSet::new(); token_list.len() - 1]);
                        let token_list = TokenList::new(token_list, false, false);
                        let references = HashSet::from([MatcherReference::Subtable(*tid)]);

                        Matcher::new(conjunctions, token_list, references)
                    })
                    .collect_vec()
            }
            MOIRSymbolId::Field(fid) => {
                // create a wildcard field matcher to extract value later on
                let field = self
                    .spec_builder
                    .field_by_symbol_id(*fid)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing field mapping")
                    })?;

                let field_matcher = FieldMatcher::Wildcard;
                let atom = MatcherAtom::Field { field: *fid, matcher: field_matcher };
                let tokens = TokenList::new(vec![field.parent], false, false);

                vec![Matcher::new(
                    vec![HashSet::from([atom])],
                    tokens,
                    HashSet::from([MatcherReference::Field(*fid)]),
                )]
            }
            MOIRSymbolId::VarNode(vnid) => {
                // include in references
                let conjunctions = vec![HashSet::new()];
                let token_list = TokenList::new(vec![MOIRFieldParent::Token(zero_token)], false, false);
                let references = HashSet::from([MatcherReference::Varnode(*vnid)]);

                vec![Matcher::new(conjunctions, token_list, references)]
            }
            _ => todo!("Generate error report for invalid symbol type"),
        };

        Ok(matchers)
    }

    fn process_binary_op(
        &self,
        lhs: Box<Spanned<SASTBitPatternConstraint>>,
        op: Spanned<SASTBinaryOp>,
        rhs: Box<Spanned<SASTBitPatternConstraint>>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        // Binary op case (mode=1 & op=0x7). Or expressions will be split and Concat
        // expressions verified, replaced by & and local tokens will be assigned.

        let mut matchers_lhs = self.create_matchers_recursive(*lhs)?;
        let mut matchers_rhs = self.create_matchers_recursive(*rhs)?;

        let new_matchers = match op.into_value() {
            SASTBinaryOp::And => {
                let mut new_matchers = Vec::new();

                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        // We first extract everything context-related into an extra matcher that
                        // will be merged at the end. This is necessary since the Sleigh writers
                        // decided to use & and not ; to separate context from regular tokens.
                        let mut matcher_lhs_nc = matcher_lhs.clone();
                        let mut matcher_rhs_nc = matcher_rhs.clone();
                        let context_matcher = extract_context(&mut matcher_lhs_nc, Some(&mut matcher_rhs_nc));

                        // Make sure tokens are the same (since lhs and rhs can be inconsistent in
                        // case a subtable with multiple constructors was used). If this is the
                        // case the resulting matcher can be discarded.
                        if let Ok(merged_tokens) =
                            self.merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list)
                        {
                            let mut token_list = context_matcher.token_list;
                            token_list.list.append(&mut merged_tokens.to_vec());
                            token_list.remove_padding();

                            // this should only be reported as an error if no compatible token
                            // lists exist for all lhs and rhs matchers
                            // TODO: todo!("Generate warning for inconsistent tokens.");

                            let mut conjunctions = context_matcher.conjunctions;
                            conjunctions.append(&mut merge_conjunctions(
                                &matcher_lhs_nc.conjunctions,
                                &matcher_rhs_nc.conjunctions,
                            ));

                            let mut references = matcher_lhs.references.clone();
                            references.extend(matcher_rhs.references.clone().into_iter());

                            new_matchers.push(Matcher::new(conjunctions, token_list, references));
                        }
                    }
                }

                new_matchers
            }
            SASTBinaryOp::Or => {
                // TODO is this correct? what do we need to check here?
                // make sure tokens are the same (since lhs and rhs are consistent themselves *in
                // the Or case*, it's enough to check the first elements) taking left/right
                // justification into account

                // We only extract the context to make sure tokens match; the conjunctions as well
                // as symbols used stay the same.
                let mut matcher_lhs_nc = matchers_lhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .clone();
                let mut matcher_rhs_nc = matchers_rhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .clone();
                let context_lhs = extract_context(&mut matcher_lhs_nc, None);
                let context_rhs = extract_context(&mut matcher_rhs_nc, None);

                let mut merged_tokens = self
                    .merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list)?
                    .to_vec();

                // Add missing conjunction/symbol lists in case the resulting token list is
                // longer than the original list. This is not necessary for the other cases
                // since everything is merged into one which we do not do here.
                matchers_lhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_lhs.token_list.clone();
                    token_list.list.append(&mut merged_tokens.clone());
                    token_list.remove_padding();
                    for _ in matcher.token_list.list.len()..token_list.list.len() {
                        matcher.conjunctions.push(HashSet::new());
                    }
                    matcher.token_list = token_list;
                });

                matchers_rhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_rhs.token_list.clone();
                    token_list.list.append(&mut merged_tokens);
                    token_list.remove_padding();
                    for _ in matcher.token_list.list.len()..token_list.list.len() {
                        matcher.conjunctions.push(HashSet::new());
                    }
                    matcher.token_list = token_list;
                });

                // combining them is done by union
                matchers_lhs.append(&mut matchers_rhs);
                matchers_lhs
            }
            SASTBinaryOp::Concatenate => {
                // same as And but we concatenate the token lists and conjunctions
                // instead of merging them

                let mut new_matchers = Vec::new();

                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        let mut matcher_lhs_nc = matcher_lhs.clone();
                        let mut matcher_rhs_nc = matcher_rhs.clone();
                        let context_matcher = extract_context(&mut matcher_lhs_nc, Some(&mut matcher_rhs_nc));

                        let mut conjunctions = context_matcher.conjunctions.clone();
                        conjunctions.append(&mut matcher_lhs_nc.conjunctions.clone());
                        conjunctions.append(&mut matcher_rhs_nc.conjunctions.clone());

                        let mut token_list = context_matcher.token_list.clone();
                        token_list
                            .list
                            .append(&mut matcher_lhs_nc.token_list.list.clone());
                        token_list
                            .list
                            .append(&mut matcher_rhs_nc.token_list.list.clone());
                        token_list.remove_padding();

                        let mut references = matcher_lhs.references.clone();
                        references.extend(matcher_rhs.references.clone().into_iter());

                        new_matchers.push(Matcher::new(conjunctions, token_list, references));
                    }
                }

                new_matchers
            }
        };

        Ok(new_matchers)
    }

    fn process_unary_op(
        &self,
        op: Spanned<SASTUnaryOp>,
        operand: Box<Spanned<SASTBitPatternConstraint>>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let mut matchers = self.create_matchers_recursive(*operand)?;
        // append/prepend undetermined token to each matcher
        match op.into_value() {
            SASTUnaryOp::JustifyLeft => {
                for matcher in matchers.iter_mut() {
                    matcher.token_list.justify_left = true;
                }
            }
            SASTUnaryOp::JustifyRight => {
                for matcher in matchers.iter_mut() {
                    matcher.token_list.justify_right = true;
                }
            }
        }

        Ok(matchers)
    }

    fn process_advanced_constraint(
        &self,
        lhs: Spanned<SASTIdent>,
        op: Spanned<SASTConstraintOp>,
        pattern: Spanned<SASTConstraintPattern>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let lhs_field_id = match self.spec_builder.get_symbol(&lhs.clone().transmute())? {
            MOIRSymbolId::Field(fid) => *fid,
            _ => todo!("Generate error report for invalid lhs of advanced constraint"),
        };

        let (conjunctions, references) =
            self.matcher_conjunctions_from_advanced_constraint(lhs_field_id, op.into_value(), pattern.into_value())?;

        let lhs_field = self
            .spec_builder
            .field_by_symbol_id(lhs_field_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing field") })?;

        let token_list = TokenList::new(vec![lhs_field.parent], false, false);

        // one matcher per conjunction returned
        Ok(conjunctions
            .into_iter()
            .map(|c| Matcher::new(vec![c], token_list.clone(), references.clone()))
            .collect::<Vec<_>>())
    }

    /// Create multiple conjunctions from a [SASTBitPatternConstraint::AdvancedConstraint]
    /// by splitting advanced constraints like negation and partial order relations into multiple
    /// bit patterns that can be matched against more easily. Be careful since this operation might
    /// create an extremely large number of matcher conjunctions.
    fn matcher_conjunctions_from_advanced_constraint(
        &self,
        field_id: FieldId,
        op: SASTConstraintOp,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<(Vec<HashSet<MatcherAtom>>, HashSet<MatcherReference>)> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for symbol undeclared") })?;

        let mut references = HashSet::new();
        references.push(MatcherReference::Field(field_id));

        let conjunctions = match pattern {
            SASTConstraintPattern::Ident(ident) => {
                let other_id = self.spec_builder.field_id_by_ident(&ident.transmute())?;
                references.push(MatcherReference::Field(other_id));
                let other = self
                    .spec_builder
                    .field_by_symbol_id(other_id)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for wrong symbol type or missing symbol")
                    })?;

                if other.bits.size() != field.bits.size() {
                    todo!("Generate error report for mismatching field sizes");
                }

                match op {
                    SASTConstraintOp::Equals => self.matcher_conjunctions_from_field_equals_constraint(field, field_id, other),
                    SASTConstraintOp::NotEquals => self.matcher_conjunctions_from_field_neq_constraint(field, field_id, other),
                    SASTConstraintOp::LessThan => self.matcher_conjunctions_from_field_gt_constraint(field, field_id, other),
                    SASTConstraintOp::GreaterThan => self.matcher_conjunctions_from_field_lt_constraint(field, field_id, other),
                }
            }
            SASTConstraintPattern::BitPattern(value) => match op {
                SASTConstraintOp::Equals => {
                    vec![self.matcher_conjunction_from_equals_constraint(field, field_id, value.into_value())]
                }
                SASTConstraintOp::NotEquals => self.matcher_conjunctions_from_neq_constraint(field, field_id, value.into_value()),
                SASTConstraintOp::LessThan => self.matcher_conjunctions_from_lt_constraint(field, field_id, value.into_value()),
                SASTConstraintOp::GreaterThan => {
                    self.matcher_conjunctions_from_gt_constraint(field, field_id, value.into_value())
                }
            },
        };

        Ok((conjunctions, references))
    }

    // Other constraints than equals are possible which have to be broken down by explicitly
    // enumerating all possible values (value pairs for field comparisons). This operation can be
    // very expensive depending on the size of the field.

    /// Generate multiple matcher conjunctions by enumerating all possible cases where the field
    /// `field` is equal to the field `other`.
    fn matcher_conjunctions_from_field_equals_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        other: &MOIRField,
    ) -> Vec<HashSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow(field.bits.size() as u32));
        (0u64..2u64.pow(field.bits.size() as u32))
            .zip(0u64..2u64.pow(other.bits.size() as u32))
            .for_each(|(v0, v1)| {
                let mask = field.bits.mask_shift() | other.bits.mask_shift();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field { field: field_id, matcher: field_matcher };
                conjunctions.push(HashSet::from([atom]));
            });

        conjunctions
    }

    // Generate multiple matcher conjunctions by enumerating all possible cases where the field `field`
    // is not equal to the field `other`.
    fn matcher_conjunctions_from_field_neq_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        other: &MOIRField,
    ) -> Vec<HashSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow((field.bits.size() + other.bits.size()) as u32));
        for v0 in 0u64..2u64.pow(field.bits.size() as u32) {
            for v1 in 0u64..2u64.pow(other.bits.size() as u32) {
                if v0 == v1 {
                    continue;
                }

                let mask = field.bits.mask_shift() | other.bits.mask_shift();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field { field: field_id, matcher: field_matcher };
                conjunctions.push(HashSet::from([atom]));
            }
        }

        conjunctions
    }

    /// Generate multiple matcher conjunctions by enumerating all possible cases where the field
    /// `field` is less than than the field `other`.
    fn matcher_conjunctions_from_field_lt_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        other: &MOIRField,
    ) -> Vec<HashSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow((field.bits.size() + other.bits.size()) as u32));
        for v0 in 0u64..2u64.pow(field.bits.size() as u32) {
            for v1 in v0..2u64.pow(other.bits.size() as u32) {
                let mask = field.bits.mask_shift() | other.bits.mask_shift();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field { field: field_id, matcher: field_matcher };
                conjunctions.push(HashSet::from([atom]));
            }
        }

        conjunctions
    }

    /// Generate multiple matcher conjunctions by enumerating all possible cases where the field
    /// `field` is greater than the field `other`.
    fn matcher_conjunctions_from_field_gt_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        other: &MOIRField,
    ) -> Vec<HashSet<MatcherAtom>> {
        self.matcher_conjunctions_from_field_lt_constraint(other, field_id, field)
    }

    /// Generate multiple matcher conjunctions by enumerating all possible cases where the fields
    /// are not equal.
    fn matcher_conjunction_from_equals_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> HashSet<MatcherAtom> {
        let mask = field.bits.mask_shift();
        let value = value << field.bits.start;
        let field_matcher = FieldMatcher::Value { mask, value };
        let atom = MatcherAtom::Field { field: field_id, matcher: field_matcher };

        HashSet::from([atom])
    }

    /// Generate multiple matcher conjunctions matching on every possible
    /// value of field except the provided value.
    fn matcher_conjunctions_from_neq_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v != value {
                    Some(self.matcher_conjunction_from_equals_constraint(field, field_id, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Generate multiple matcher atoms matching on every possible
    /// value of field except values not less than the provided value.
    fn matcher_conjunctions_from_lt_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v < value {
                    Some(self.matcher_conjunction_from_equals_constraint(field, field_id, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Generate an or chain of matcher atoms matching on every possible
    /// value of field except values not greater than the provided value.
    fn matcher_conjunctions_from_gt_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v > value {
                    Some(self.matcher_conjunction_from_equals_constraint(field, field_id, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Merge two token lists by checking whether they are compatible with one another, i.e. if they
    /// are equal taking potential left/right justification for different lengths into account. Two
    /// lists are always compatible if one of them is empty.

    /// # Return
    /// - Ok(list) if the lists are compatible with `list` being the more specific (i.e. longer) one
    /// - Err() if the lists are not compatible
    fn merge_token_lists<'a>(
        &self,
        lhs: &'a TokenList,
        rhs: &'a TokenList,
    ) -> SleighCompilerResult<&'a [MOIRFieldParent]> {
        // lists can only be empty temporarily when their context is extracted, otherwise
        // they always have at least the zero token
        if lhs.list.is_empty()  || rhs.list.is_empty(){
            return Ok(&rhs.list);
        } else if rhs.list.is_empty() {
            return Ok(&lhs.list);
        }

        // special case: one of the lists consists of the empty token (semantically the same as above;
        // in addition, there can't be a token list of length greater than one containing the empty
        // token since it is eliminated on merge)
        let empty_token = MOIRFieldParent::Token(
            self.spec_builder
                .zero_token()
                .ok_or_else(|| -> SleighCompilerError { todo!("The zero token has to be defined at this point") })?,
        );
        if lhs.list[0] == empty_token {
            return Ok(&rhs.list);
        } else if rhs.list[0] == empty_token {
            return Ok(&lhs.list);
        }

        if lhs.justify_left && rhs.justify_left {
            // find most common suffix
            let i = lhs.list.len().min(rhs.list.len());
            return Ok(&lhs.list[(lhs.list.len() - i)..]);
        }

        if lhs.justify_right && rhs.justify_right {
            // find most common prefix
            let i = lhs.list.len().min(rhs.list.len());
            return Ok(&lhs.list[..i]);
        }

        // padded one contained in longer? (start/end/middle)
        let (padded, non_padded) = if lhs.justify_right || lhs.justify_left {
            (lhs, rhs)
        } else {
            (rhs, lhs)
        };

        if (padded.justify_left || padded.justify_right) && padded.list.len() > non_padded.list.len() {
            todo!("Generate error report for padded length greater than non-padded length");
        }

        if padded.justify_right && padded.justify_left {
            // both padded => is padded list contained in non_padded?
            for si in 0..(non_padded.list.len() - padded.list.len()) {
                if &padded.list[..] == &non_padded.list[si..(si + padded.list.len())] {
                    return Ok(&non_padded.list);
                }
            }
        } else if padded.justify_right {
            // start padded => is smaller list present at end of larger?
            if &padded.list[..] == &non_padded.list[(non_padded.list.len() - padded.list.len())..] {
                return Ok(&non_padded.list);
            }
        } else if padded.justify_left {
            // end padded => is smaller list present at start of larger?
            if &padded.list[..] == &non_padded.list[..padded.list.len()] {
                return Ok(&non_padded.list);
            }
        }

        // nothing padded => regular full-length search
        if padded == non_padded {
            Ok(&non_padded.list)
        } else {
            Err(SleighCompilerError::TODO)
        }
    }
}

/// Removes all [MOIRFieldParent::Context] tokens as well as token fields from `lhs` and `rhs` and
/// returns them in a new list with duplicates removed. This is a helper function which assumes the
/// resulting matchers are merged again some time later so references are not set, yielding
/// inconsistent matchers.
fn extract_context(
    lhs: &mut Matcher,
    rhs_maybe: Option<&mut Matcher>,
) -> Matcher {
    let context_lhs = lhs
        .token_list
        .list
        .iter()
        .zip(lhs.conjunctions.iter())
        .filter_map(|(token, conjunction)| match token {
            MOIRFieldParent::Context(_) => Some((token.clone(), conjunction.clone())),
            MOIRFieldParent::Token(_) => None,
        });

    let mut context_rhs = if let Some(ref rhs) = rhs_maybe {
        rhs.token_list
            .list
            .iter()
            .zip(rhs.conjunctions.iter())
            .filter_map(|(token, conjunction)| match token {
                MOIRFieldParent::Context(_) => Some((token.clone(), conjunction.clone())),
                MOIRFieldParent::Token(_) => None,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // merge context_lhs into context_rhs and create context matcher
    'next: for (token, conjunctions) in context_lhs {
        for (token_rhs, conjunctions_rhs) in context_rhs.iter_mut() {
            if token == *token_rhs {
                conjunctions_rhs.extend(conjunctions);
                break 'next;
            }
        }

        context_rhs.push((token.clone(), conjunctions.clone()));
    }

    let (context_tokens, context_conjunctions): (Vec<_>, Vec<_>) = context_rhs.into_iter().unzip();
    let context_matcher =
        Matcher::new(context_conjunctions, TokenList::new(context_tokens, false, false), HashSet::new());

    let lhs_new = lhs
        .token_list
        .list
        .iter()
        .zip(lhs.conjunctions.iter())
        .filter_map(|(token, conjunction)| match token {
            MOIRFieldParent::Context(_) => None,
            MOIRFieldParent::Token(_) => Some((token.clone(), conjunction.clone())),
        });

    let (lhs_tokens, lhs_conjunctions): (Vec<_>, Vec<_>) = lhs_new.into_iter().unzip();
    *lhs = Matcher::new(
        lhs_conjunctions,
        TokenList {
            list: lhs_tokens,
            ..lhs.token_list
        },
        HashSet::new(),
    );

    if let Some(rhs) = rhs_maybe {
        let rhs_new = rhs
            .token_list
            .list
            .iter()
            .zip(rhs.conjunctions.iter())
            .filter_map(|(token, conjunction)| match token {
                MOIRFieldParent::Context(_) => None,
                MOIRFieldParent::Token(_) => Some((token.clone(), conjunction.clone())),
            });

        let (rhs_tokens, rhs_conjunctions): (Vec<_>, Vec<_>) = rhs_new.into_iter().unzip();
        *rhs = Matcher::new(
            rhs_conjunctions,
            TokenList {
                list: rhs_tokens,
                ..rhs.token_list
            },
            HashSet::new(),
        );
    }

    context_matcher
}

/// Merge two symbol lists by combining sets at identical positions through union and extending
/// the smaller list with elements from the larger one.
fn merge_symbols_used(
    lhs: &Vec<HashSet<FieldId>>,
    rhs: &Vec<HashSet<FieldId>>,
) -> Vec<HashSet<FieldId>> {
    let mut symbols_combined = lhs.clone();
    for (i, symbols_rhs) in rhs.iter().enumerate() {
        if let Some(symbols) = symbols_combined.get_mut(i) {
            symbols.extend(symbols_rhs);
        } else {
            symbols_combined.push(symbols_rhs.clone());
        }
    }

    symbols_combined
}

/// Merge two conjunction lists by combining conjunctions at identical positions through appending and extending
/// the smaller list with elements from the larger one.
fn merge_conjunctions(
    lhs: &Vec<HashSet<MatcherAtom>>,
    rhs: &Vec<HashSet<MatcherAtom>>,
) -> Vec<HashSet<MatcherAtom>> {
    let mut conjunctions_combined = lhs.clone();

    for (i, conjunctions_rhs) in rhs.iter().enumerate() {
        if let Some(conjunctions) = conjunctions_combined.get_mut(i) {
            conjunctions.extend(conjunctions_rhs.clone());
        } else {
            conjunctions_combined.push(conjunctions_rhs.clone());
        }
    }

    conjunctions_combined
}
