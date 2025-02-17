use std::collections::{
    BTreeMap,
    BTreeSet,
    HashSet,
};

use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    MatcherReference,
    TokenList,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::InternalError;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::constructor::ConstructorId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::moir::table::TableId;
use itertools::Itertools;
use mogi_sleigh_parser::Spanned;
use mogi_sleigh_parser::ast::common::SASTIdent;
use mogi_sleigh_parser::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
use num::PrimInt;

use super::matcher::Submatchers;

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
        constraint: &Spanned<SASTBitPatternConstraint>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        let matchers = self.create_matchers_recursive(constraint, table_id, constructor_id)?;

        Ok(matchers)
    }

    /// Recursive entry called through [Self::create_matchers]
    fn create_matchers_recursive(
        &self,
        constraint: &Spanned<SASTBitPatternConstraint>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        let (constraint, _span) = constraint.as_tuple();
        let new_matchers = match constraint {
            SASTBitPatternConstraint::Ident(ident) => self.process_ident(*ident, table_id, constructor_id)?,
            SASTBitPatternConstraint::BinaryOp(lhs, op, rhs) => {
                self.process_binary_op(lhs, *op, rhs, table_id, constructor_id)?
            }
            SASTBitPatternConstraint::UnaryOp(op, operand) => {
                self.process_unary_op(*op, operand, table_id, constructor_id)?
            }
            SASTBitPatternConstraint::AdvancedConstraint(lhs, op, pattern) => {
                self.process_advanced_constraint(*lhs, *op, *pattern, table_id, constructor_id)?
            }
            SASTBitPatternConstraint::Invalid => {
                todo!("Generate error report for an invalid constraint");
            }
        };

        Ok(new_matchers)
    }

    /// Process an identifier into potentially multiple matchers by creating
    /// [Matcher]s for the tokens and symbols referenced.
    fn process_ident(
        &self,
        ident: Spanned<SASTIdent>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        // Ident case: freestanding fields, varnodes or subtables linking global to local symbols. This
        // is one of the base cases and yields multiple simple [Matcher] with [MatcherAtom::Linkage]
        // and the corresponding symbols, fields and tokens.

        let symbol = self.spec_builder.get_symbol(&ident.transmute())?;

        let matchers = match symbol {
            MOIRSymbolId::Table(tid) => {
                // all table references start as unconstrained and are only constrained
                // as needed
                let atom = MatcherAtom::Submatchers(Submatchers::new(*tid, None));
                let conjunctions = vec![BTreeSet::from([atom])];
                let references = HashSet::from([MatcherReference::Subtable(*tid)]);
                let token_list = TokenList::new(vec![None], true, false);
                vec![Matcher::new(
                    conjunctions,
                    token_list,
                    references,
                    table_id,
                    constructor_id,
                )]
            }
            MOIRSymbolId::Field(fid) => {
                // create a wildcard field matcher to extract value later on
                let field = self
                    .spec_builder
                    .field_by_symbol_id(*fid)
                    .ok_or_else(|| -> InternalError { todo!("Generate error report for missing field mapping") })?;

                let field_matcher = FieldMatcher::Wildcard;
                let atom = MatcherAtom::Field {
                    field_id: *fid,
                    matcher: field_matcher,
                };
                let tokens = TokenList::new(vec![Some(field.parent)], false, false);

                vec![Matcher::new(
                    vec![BTreeSet::from([atom])],
                    tokens,
                    HashSet::from([MatcherReference::Field(*fid)]),
                    table_id,
                    constructor_id,
                )]
            }
            MOIRSymbolId::VarNode(vnid) => {
                // include in references
                let conjunctions = Vec::new();
                let token_list = TokenList::new(Vec::new(), false, false);
                let references = HashSet::from([MatcherReference::Varnode(*vnid)]);

                vec![Matcher::new(
                    conjunctions,
                    token_list,
                    references,
                    table_id,
                    constructor_id,
                )]
            }
            _ => todo!("Generate error report for invalid symbol type"),
        };

        Ok(matchers)
    }

    fn process_binary_op(
        &self,
        lhs: &Spanned<SASTBitPatternConstraint>,
        op: Spanned<SASTBinaryOp>,
        rhs: &Spanned<SASTBitPatternConstraint>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        // Binary op case (mode=1 & op=0x7). Or expressions will be split and Concat
        // expressions verified, replaced by & and local tokens will be assigned.

        let mut matchers_lhs = self.create_matchers_recursive(lhs, table_id, constructor_id)?;
        let mut matchers_rhs = self.create_matchers_recursive(rhs, table_id, constructor_id)?;

        let new_matchers = match op.into_value() {
            SASTBinaryOp::And => {
                let mut new_matchers = Vec::new();

                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        // Special case: one of the matchers is empty (this can happen
                        // if the bit pattern contains only `epsilon`). In this case,
                        // we'll just use the other matcher as-is

                        // We first extract everything context-related into an extra matcher that
                        // will be merged at the end. This is necessary since the Sleigh writers
                        // decided to use & and not ; to separate context from regular tokens.
                        let mut matcher_lhs_nc = matcher_lhs.clone();
                        let mut matcher_rhs_nc = matcher_rhs.clone();
                        let context_matcher = extract_context(&mut matcher_lhs_nc, Some(&mut matcher_rhs_nc));

                        // Make sure tokens are the same (since lhs and rhs can be inconsistent in
                        // case a subtable with multiple constructors was used). If this is the
                        // case the resulting matcher can be discarded.
                        if let Some(merged_tokens) =
                            merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list)
                        {
                            let mut merged_conjunctions =
                                merge_conjunctions(&matcher_lhs_nc.conjunctions, &matcher_rhs_nc.conjunctions);

                            // constrain submatchers (technically necessary but idk)
                            // self.constrain_submatchers(&mut merged_conjunctions, &merged_tokens.list);

                            let mut token_list = context_matcher.token_list;
                            token_list.list.extend(merged_tokens.list);

                            let mut conjunctions = context_matcher.conjunctions;
                            conjunctions.append(&mut merged_conjunctions);

                            let mut references = matcher_lhs.references.clone();
                            references.extend(matcher_rhs.references.clone().into_iter());

                            new_matchers.push(Matcher::new(
                                conjunctions,
                                token_list,
                                references,
                                table_id,
                                constructor_id,
                            ));
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

                let merged_tokens = match merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list) {
                    Some(merged_tokens) => merged_tokens,
                    // not compatible => abort
                    None => return Ok(vec![]),
                };

                // Add missing conjunction/symbol lists in case the resulting token list is
                // longer than the original list. This is not necessary for the other cases
                // since everything is merged into one which we do not do here.
                matchers_lhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_lhs.token_list.clone();
                    token_list.list.extend(merged_tokens.list.iter());
                    for _ in matcher.token_list.list.len()..token_list.list.len() {
                        matcher.conjunctions.push(BTreeSet::new());
                    }
                    matcher.token_list = token_list;
                });

                matchers_rhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_rhs.token_list.clone();
                    token_list.list.extend(merged_tokens.list.iter());
                    for _ in matcher.token_list.list.len()..token_list.list.len() {
                        matcher.conjunctions.push(BTreeSet::new());
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
                        token_list.list.extend(matcher_lhs_nc.token_list.list);
                        token_list.list.extend(matcher_rhs_nc.token_list.list);
                        token_list.remove_padding();

                        let mut references = matcher_lhs.references.clone();
                        references.extend(matcher_rhs.references.clone().into_iter());

                        new_matchers.push(Matcher::new(conjunctions, token_list, references, table_id, constructor_id));
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
        operand: &Spanned<SASTBitPatternConstraint>,
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        let mut matchers = self.create_matchers_recursive(operand, table_id, constructor_id)?;

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
        table_id: TableId,
        constructor_id: ConstructorId,
    ) -> InternalCompilerResult<Vec<Matcher>> {
        let lhs_field_id = match self.spec_builder.get_symbol(&lhs.clone().transmute())? {
            MOIRSymbolId::Field(fid) => *fid,
            _ => todo!("Generate error report for invalid lhs of advanced constraint"),
        };

        let (conjunctions, references) =
            self.matcher_conjunctions_from_advanced_constraint(lhs_field_id, op.into_value(), pattern.into_value())?;

        let lhs_field = self
            .spec_builder
            .field_by_symbol_id(lhs_field_id)
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing field") })?;

        let token_list = TokenList::new(vec![Some(lhs_field.parent)], false, false);

        // one matcher per conjunction returned
        Ok(conjunctions
            .into_iter()
            .map(|c| Matcher::new(vec![c], token_list.clone(), references.clone(), table_id, constructor_id))
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
    ) -> InternalCompilerResult<(Vec<BTreeSet<MatcherAtom>>, HashSet<MatcherReference>)> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or_else(|| -> InternalError { todo!("Generate error report for symbol undeclared") })?;

        let mut references = HashSet::new();
        references.insert(MatcherReference::Field(field_id));

        let conjunctions = match pattern {
            SASTConstraintPattern::Ident(ident) => {
                let other_id = self.spec_builder.field_id_by_ident(&ident.transmute())?;
                references.insert(MatcherReference::Field(other_id));
                let other = self
                    .spec_builder
                    .field_by_symbol_id(other_id)
                    .ok_or_else(|| -> InternalError {
                        todo!("Generate error report for wrong symbol type or missing symbol")
                    })?;

                if other.bits.bits() != field.bits.bits() {
                    todo!("Generate error report for mismatching field sizes");
                }

                match op {
                    SASTConstraintOp::Equals => {
                        self.matcher_conjunctions_from_field_equals_constraint(field, field_id, other)
                    }
                    SASTConstraintOp::NotEquals => {
                        self.matcher_conjunctions_from_field_neq_constraint(field, field_id, other)
                    }
                    SASTConstraintOp::LessThan => {
                        self.matcher_conjunctions_from_field_gt_constraint(field, field_id, other)
                    }
                    SASTConstraintOp::GreaterThan => {
                        self.matcher_conjunctions_from_field_lt_constraint(field, field_id, other)
                    }
                }
            }
            SASTConstraintPattern::BitPattern(value) => match op {
                SASTConstraintOp::Equals => {
                    vec![self.matcher_conjunction_from_equals_constraint(field, field_id, value.into_value())]
                }
                SASTConstraintOp::NotEquals => {
                    self.matcher_conjunctions_from_neq_constraint(field, field_id, value.into_value())
                }
                SASTConstraintOp::LessThan => {
                    self.matcher_conjunctions_from_lt_constraint(field, field_id, value.into_value())
                }
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow(field.bits.bits() as u32));
        (0u64..2u64.pow(field.bits.bits() as u32))
            .zip(0u64..2u64.pow(other.bits.bits() as u32))
            .for_each(|(v0, v1)| {
                let mask = field.bits.shifted_mask() | other.bits.shifted_mask();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field {
                    field_id,
                    matcher: field_matcher,
                };
                conjunctions.push(BTreeSet::from([atom]));
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow((field.bits.bits() + other.bits.bits()) as u32));
        for v0 in 0u64..2u64.pow(field.bits.bits() as u32) {
            for v1 in 0u64..2u64.pow(other.bits.bits() as u32) {
                if v0 == v1 {
                    continue;
                }

                let mask = field.bits.shifted_mask() | other.bits.shifted_mask();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field {
                    field_id,
                    matcher: field_matcher,
                };
                conjunctions.push(BTreeSet::from([atom]));
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        let mut conjunctions = Vec::with_capacity(2.pow((field.bits.bits() + other.bits.bits()) as u32));
        for v0 in 0u64..2u64.pow(field.bits.bits() as u32) {
            for v1 in v0..2u64.pow(other.bits.bits() as u32) {
                let mask = field.bits.shifted_mask() | other.bits.shifted_mask();
                let value = (v0 << field.bits.start) | (v1 << other.bits.start);
                let field_matcher = FieldMatcher::Value { mask, value };
                let atom = MatcherAtom::Field {
                    field_id,
                    matcher: field_matcher,
                };
                conjunctions.push(BTreeSet::from([atom]));
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        self.matcher_conjunctions_from_field_lt_constraint(other, field_id, field)
    }

    /// Generate multiple matcher conjunctions by enumerating all possible cases where the fields
    /// are not equal.
    fn matcher_conjunction_from_equals_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> BTreeSet<MatcherAtom> {
        let mask = field.bits.shifted_mask();
        let value = value << field.bits.start;
        let field_matcher = FieldMatcher::Value { mask, value };
        let atom = MatcherAtom::Field {
            field_id,
            matcher: field_matcher,
        };

        BTreeSet::from([atom])
    }

    /// Generate multiple matcher conjunctions matching on every possible
    /// value of field except the provided value.
    fn matcher_conjunctions_from_neq_constraint(
        &self,
        field: &MOIRField,
        field_id: FieldId,
        value: u64,
    ) -> Vec<BTreeSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.bits() as u32))
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.bits() as u32))
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
    ) -> Vec<BTreeSet<MatcherAtom>> {
        (0u64..2u64.pow(field.bits.bits() as u32))
            .filter_map(|v| {
                if v > value {
                    Some(self.matcher_conjunction_from_equals_constraint(field, field_id, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Eliminate all submatchers that don't fit the given token list.
    ///
    /// Submatchers start out as unconstrained but might get constrained along the way.
    /// Consider the following:
    ///
    /// `:LEA reg_a, reg_b is (byte=0x8D; mod=3) & (byte=0x8D; addr64) { ... } `
    ///
    /// `addr64` starts out as being unconstrained but then gets constrained to the tokens `[byte, mod]`.
    /// This eliminates any constructors of `addr64` that use different tokens. Since this
    /// influences matching semantics, it should be accounted for.
    fn constrain_submatchers(
        &self,
        merged_conjunctions: &mut Vec<BTreeSet<MatcherAtom>>,
        merged_tokens: &Vec<Option<MOIRFieldParent>>,
    ) {
        todo!("Constrain submatchers")
    }
}

/// Combine two tokens in the following manner:
/// - if one if them is None, return the other
/// - if both are Some, return Some(Some()) if they are the same
/// - if both are Some but are not the same, return None
fn combine_tokens(
    left: Option<MOIRFieldParent>,
    right: Option<MOIRFieldParent>,
) -> Option<Option<MOIRFieldParent>> {
    match left {
        Some(l) => match right {
            Some(r) => {
                if l == r {
                    // Some(left) and Some(right) and left == right => Some(left)
                    Some(Some(l))
                } else {
                    // Some(left) and Some(right) and left != right => None
                    None
                }
            }
            // Some(left) and None(right) => Some(left)
            None => Some(Some(l)),
        },
        // None(left) => Some(right)
        None => Some(right),
    }
}

/// Merge two token lists by checking whether they are compatible with one another, i.e. if they
/// are equal taking potential left/right justification for different lengths into account, and
/// by filling potential None tokens. Two lists are always compatible if one of them is empty.
/// The resulting list is guaranteed to not contain any None tokens.
///
/// # Return
/// - Some(list) if the lists are compatible with `list` being the more specific (i.e. longer) one
/// - None if the lists are not compatible
pub(crate) fn merge_token_lists(
    lhs: &TokenList,
    rhs: &TokenList,
) -> Option<TokenList> {
    // only one can be justified either left or right or both
    debug_assert!(!(lhs.justify_left && lhs.justify_right));
    debug_assert!(!(rhs.justify_left && rhs.justify_right));
    // debug_assert!(!(lhs.justify_left && rhs.justify_left));
    // debug_assert!(!(rhs.justify_right && lhs.justify_right));

    // return if empty or just None (this is a special case, the more general one
    // is not yet implemented)
    if lhs.list.is_empty() || lhs.list.len() == 1 && lhs.list[0] == None {
        return Some(rhs.clone());
    } else if rhs.list.is_empty() || rhs.list.len() == 1 && rhs.list[0] == None {
        return Some(lhs.clone());
    }

    // - if both are non-justified, the order doesn't matter
    // - if one is justified but the other is not, list_0 equals the justified one
    // - if both are justified, list_0 equals the longer one

    let left_justified = lhs.justify_left || lhs.justify_right;
    let right_justified = rhs.justify_left || rhs.justify_right;

    let (list_0, list_1) = if left_justified && right_justified {
        // both are justified
        if lhs.list.len() > rhs.list.len() {
            // both are justified, but left is longer
            (lhs, rhs)
        } else {
            // both are justified, but right is longer
            (rhs, lhs)
        }
    } else if left_justified {
        // left is justified
        (lhs, rhs)
    } else {
        // right may or may not be justified
        (rhs, lhs)
    };

    let mut result_list = Vec::new();

    if list_0.justify_left {
        // left-justified => find common prefix while combining None tokens
        for i in 0..list_0.list.len().max(list_1.list.len()) {
            let token_lhs = list_0
                .list
                .get(i)
                .map(|inner| inner.as_ref().copied())
                .flatten();
            let token_rhs = list_1
                .list
                .get(i)
                .map(|inner| inner.as_ref().copied())
                .flatten();
            match combine_tokens(token_lhs, token_rhs) {
                Some(token) => result_list.push(token),
                None => return None,
            }
        }
    } else if list_0.justify_right {
        // right-justified => find common suffix while combining None tokens
        for i in 0..list_0.list.len().max(list_1.list.len()) {
            let token_lhs = list_0
                .list
                .get(lhs.list.len() - i - 1)
                .map(|inner| inner.as_ref().copied())
                .flatten();
            let token_rhs = list_1
                .list
                .get(rhs.list.len() - i - 1)
                .map(|inner| inner.as_ref().copied())
                .flatten();
            match combine_tokens(token_lhs, token_rhs) {
                Some(token) => result_list.push(token),
                None => return None,
            }
        }

        result_list.reverse();
    } else {
        // nothing is justified => lists must be the same
        if list_0.list.len() != list_1.list.len() {
            return None;
        }

        for (&token_lhs, &token_rhs) in list_0.list.iter().zip(list_1.list.iter()) {
            match combine_tokens(token_lhs, token_rhs) {
                Some(token) => result_list.push(token),
                None => return None,
            }
        }
    }

    Some(TokenList::new(result_list, false, false))
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
        .filter_map(|(token, conjunction)| {
            token.and_then(|token| match token {
                MOIRFieldParent::Context(_) => Some((Some(token.clone()), conjunction.clone())),
                MOIRFieldParent::Token(_) => None,
            })
        });

    let mut context_rhs = if let Some(ref rhs) = rhs_maybe {
        rhs.token_list
            .list
            .iter()
            .zip(rhs.conjunctions.iter())
            .filter_map(|(token, conjunction)| {
                token.and_then(|token| match token {
                    MOIRFieldParent::Context(_) => Some((Some(token.clone()), conjunction.clone())),
                    MOIRFieldParent::Token(_) => None,
                })
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
    let context_matcher = Matcher::new(
        context_conjunctions,
        TokenList::new(context_tokens, false, false),
        HashSet::new(),
        lhs.table_id,
        lhs.constructor_id,
    );

    let lhs_new = lhs
        .token_list
        .list
        .iter()
        .zip(lhs.conjunctions.iter())
        .filter_map(|(token, conjunction)| {
            if matches!(token, Some(MOIRFieldParent::Context(_))) {
                None
            } else {
                Some((token.clone(), conjunction.clone()))
            }
        });

    let (lhs_tokens, lhs_conjunctions): (Vec<_>, Vec<_>) = lhs_new.into_iter().unzip();
    *lhs = Matcher::new(
        lhs_conjunctions,
        TokenList {
            list: lhs_tokens,
            ..lhs.token_list
        },
        HashSet::new(),
        lhs.table_id,
        lhs.constructor_id,
    );

    if let Some(rhs) = rhs_maybe {
        let rhs_new = rhs
            .token_list
            .list
            .iter()
            .zip(rhs.conjunctions.iter())
            .filter_map(|(token, conjunction)| {
                if matches!(token, Some(MOIRFieldParent::Context(_))) {
                    None
                } else {
                    Some((token.clone(), conjunction.clone()))
                }
            });

        let (rhs_tokens, rhs_conjunctions): (Vec<_>, Vec<_>) = rhs_new.into_iter().unzip();
        *rhs = Matcher::new(
            rhs_conjunctions,
            TokenList {
                list: rhs_tokens,
                ..rhs.token_list
            },
            HashSet::new(),
            rhs.table_id,
            rhs.constructor_id,
        );
    }

    context_matcher
}

/// Merge two conjunction lists by combining conjunctions at identical positions through appending and extending
/// the smaller list with elements from the larger one.
fn merge_conjunctions(
    lhs: &Vec<BTreeSet<MatcherAtom>>,
    rhs: &Vec<BTreeSet<MatcherAtom>>,
) -> Vec<BTreeSet<MatcherAtom>> {
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
