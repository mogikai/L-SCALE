use std::collections::{
    BTreeSet,
    HashSet,
};

use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    MatcherReference,
    MatcherToken,
};
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

use super::matcher::{
    MatcherConjunction,
    Submatchers,
};

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
                let token = MatcherToken::new(None);
                let conjunction = MatcherConjunction::new(BTreeSet::from([atom]), token);
                let references = HashSet::from([MatcherReference::Subtable(*tid)]);
                vec![Matcher::new(vec![conjunction], references, table_id, constructor_id)]
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
                let token = MatcherToken::new(Some(field.parent));
                let conjunction = MatcherConjunction::new(BTreeSet::from([atom]), token);

                vec![Matcher::new(
                    vec![conjunction],
                    HashSet::from([MatcherReference::Field(*fid)]),
                    table_id,
                    constructor_id,
                )]
            }
            MOIRSymbolId::VarNode(vnid) => {
                // include in references
                let references = HashSet::from([MatcherReference::Varnode(*vnid)]);
                let conjunction = MatcherConjunction::default();

                vec![Matcher::new(vec![conjunction], references, table_id, constructor_id)]
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
                        // We first extract context conjunctions that will be merged at the end.
                        // This is necessary since the Sleigh writers decided to use & and not ; to
                        // separate context from regular tokens.
                        let mut matcher_lhs_nc = matcher_lhs.clone();
                        let mut matcher_rhs_nc = matcher_rhs.clone();
                        let mut context_conjunctions = extract_context(&mut matcher_lhs_nc, Some(&mut matcher_rhs_nc));

                        // Make sure tokens are the same (since lhs and rhs can be inconsistent in
                        // case a subtable with multiple constructors was used). If this is the
                        // case the resulting matcher can be discarded.
                        if let Some(merged_conjunctions) =
                            merge_conjunctions(true, &matcher_lhs_nc.conjunctions, &matcher_rhs_nc.conjunctions)
                        {
                            let mut merged_conjunctions = merged_conjunctions
                                .into_iter()
                                .map(|(mut lhs, mut rhs)| {
                                    lhs.atoms.append(&mut rhs.atoms);
                                    lhs.token.parent = lhs.token.parent.or(rhs.token.parent);
                                    lhs
                                })
                                .collect_vec();

                            context_conjunctions.append(&mut merged_conjunctions);

                            let mut references = matcher_lhs.references.clone();
                            references.extend(matcher_rhs.references.clone().into_iter());

                            new_matchers.push(Matcher::new(context_conjunctions, references, table_id, constructor_id));
                        }
                    }
                }

                new_matchers
            }
            SASTBinaryOp::Or => {
                // TODO this is not correct since the Or operator can force the
                // token list to change, but what do we need to check exactly?
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
                        let context_conjunctions = extract_context(&mut matcher_lhs_nc, Some(&mut matcher_rhs_nc));

                        // let concretized_lhs_nc = self.concretize_matcher(&matcher_lhs_nc.conjunctions);

                        let mut references = matcher_lhs.references.clone();
                        references.extend(matcher_rhs.references.clone().into_iter());

                        let mut conjunctions = context_conjunctions.clone();
                        conjunctions.append(&mut matcher_lhs_nc.conjunctions);
                        conjunctions.append(&mut matcher_rhs_nc.conjunctions);

                        new_matchers.push(Matcher::new(conjunctions, references.clone(), table_id, constructor_id));
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

        // append/prepend None token to each matcher
        match op.into_value() {
            SASTUnaryOp::JustifyLeft => {
                for matcher in matchers.iter_mut() {
                    matcher
                        .conjunctions
                        .push(MatcherConjunction::new(BTreeSet::default(), MatcherToken::new(None)));
                }
            }
            SASTUnaryOp::JustifyRight => {
                for matcher in matchers.iter_mut() {
                    matcher
                        .conjunctions
                        .insert(0, MatcherConjunction::new(BTreeSet::default(), MatcherToken::new(None)));
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

        // one matcher per conjunction returned
        Ok(conjunctions
            .into_iter()
            .map(|c| Matcher::new(vec![c], references.clone(), table_id, constructor_id))
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
    ) -> InternalCompilerResult<(Vec<MatcherConjunction>, HashSet<MatcherReference>)> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or_else(|| -> InternalError { todo!("Generate error report for symbol undeclared") })?;

        let token = MatcherToken::new(Some(field.parent));

        let mut references = HashSet::new();
        references.insert(MatcherReference::Field(field_id));

        let atoms = match pattern {
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

        let conjunctions = atoms
            .into_iter()
            .map(|atoms| MatcherConjunction::new(atoms, token.clone()))
            .collect_vec();

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

    /// Find all possible token lists of `matcher` that are compatible with `root_constraint`.
    fn concretize_matcher(
        &self,
        matcher: &[MatcherConjunction],
    ) -> Vec<Vec<MatcherConjunction>> {
        todo!("");

        // // start with matcher
        // let mut result = vec![matcher.to_vec()];

        // // root_constraint = t0; None; t2; None

        // // t0; None; t2; None

        // // t0; imm16; t2; None
        // // t0; imm32; t2; None

        // // t0; modrm; t2; byte
        // // t0; modrm; t2; sib

        // for token_start in 0..matcher.len() {
        //     // new values are added each iteration

        //     let mut new_results = Vec::new();
        //     for result_i in 0..result.len() {
        //         // start off from previous results
        //         let prev_result = &result[result_i];
        //         let slice = &prev_result[token_start..];
        //         let start = slice.first().expect("There must be at least one");

        //         // we need to concretize every submatcher because if we encounter a None token it might
        //         // be already too late since submatchers from earlier can constrain it
        //         for atom in &start.atoms {
        //             if let MatcherAtom::Submatchers(submatchers) = atom {
        //                 let table = self
        //                     .spec_builder
        //                     .table_by_symbol_id(submatchers.table)
        //                     .expect("Invalid table reference");
        //                 let matchers = match &submatchers.matchers {
        //                     Some(matcher_ids) => matcher_ids
        //                         .iter()
        //                         .map(|matcher_id| {
        //                             table
        //                                 .matchers
        //                                 .get(*matcher_id)
        //                                 .expect("Invalid matcher reference")
        //                         })
        //                         .collect_vec(),
        //                     None => table.matchers.values().collect_vec(),
        //                 };

        //                 // pre-filter matchers by merging them with the root constraint
        //                 // (this needs to be done again after concretization)
        //                 let filtered_matchers = matchers.into_iter().filter_map(|matcher| {
        //                     match merge_conjunctions(&matcher.conjunctions, slice) {
        //                         Some(merged) => {
        //                             let (merged_matcher, _): (Vec<MatcherConjunction>, Vec<MatcherConjunction>) =
        //                                 merged.into_iter().unzip();
        //                             Some(merged_matcher)
        //                         }
        //                         _ => None,
        //                     }
        //                 });

        //                 // recursively concretize filtered matchers
        //                 let mut concretized_submatchers = filtered_matchers
        //                     .map(|matcher| {
        //                         // concretization is recursive
        //                         self.concretize_matcher(&matcher)
        //                     })
        //                     .flatten()
        //                     .collect_vec();

        //                 // add to result
        //                 new_results.append(&mut concretized_submatchers);
        //             }
        //         }
        //     }

        //     result = new_results;
        // }

        // result
    }
}

/// Combine two tokens in the following manner:
/// - if one if them is None, return the other
/// - if both are Some, return Some(Some()) if they are the same
/// - if both are Some but are not the same, return None
fn combine_tokens(
    left: &MatcherToken,
    right: &MatcherToken,
) -> Option<MatcherToken> {
    match left.parent {
        Some(l) => match right.parent {
            Some(r) => {
                if l == r {
                    // Some(left) and Some(right) and left == right => Some(left)
                    Some(MatcherToken::new(Some(l)))
                } else {
                    // Some(left) and Some(right) and left != right => None
                    None
                }
            }
            // Some(left) and None(right) => Some(left)
            None => Some(MatcherToken::new(Some(l))),
        },
        // None(left) => Some(right)
        None => Some(MatcherToken::new(right.parent)),
    }
}

// C0; C1; T0; T1
// C1; T0; T1
//
// No; No; T0; T1
// No; T0; T1

/// Merge two conjunctions by checking whether they are compatible with one another, i.e. if they
/// are equal taking potential left/right justification for different lengths into account, and by
/// filling potential None tokens. Two lists are always compatible if one of them is empty. Return
/// a list of conjunction pairs with identical tokens but different atoms from lhs and rhs respectively.
///
/// If `accept_empty` is set to true, an empty list will lead to the other being returned.
pub(crate) fn merge_conjunctions(
    accept_empty: bool,
    lhs: &[MatcherConjunction],
    rhs: &[MatcherConjunction],
) -> Option<Vec<(MatcherConjunction, MatcherConjunction)>> {
    // base case: both are empty
    if lhs.is_empty() && rhs.is_empty() {
        return Some(vec![]);
    } else if lhs.is_empty() && accept_empty {
        return Some(
            rhs.iter()
                .map(|c| (MatcherConjunction::default(), c.clone()))
                .collect(),
        );
    } else if rhs.is_empty() && accept_empty {
        return Some(
            lhs.iter()
                .map(|c| (c.clone(), MatcherConjunction::default()))
                .collect(),
        );
    }

    let parent_lhs = lhs.first()?.token.parent;
    let parent_rhs = rhs.first()?.token.parent;

    if parent_lhs.is_some_and(|token_lhs| parent_rhs.is_some_and(|token_rhs| token_lhs == token_rhs)) {
        // both are some and equal => merge conjunctions and recurse
        let mut result_list = vec![(lhs[0].clone(), rhs[0].clone())];

        match merge_conjunctions(false, &lhs[1..], &rhs[1..]) {
            Some(mut recurse_list) => {
                // got merge
                result_list.append(&mut recurse_list);
                return Some(result_list);
            }
            None => return None,
        }
    } else if parent_lhs.is_none() {
        // token_lhs is None => try to absorb 0 to rhs.len() tokens from rhs and recurse with that
        // .*;     t1; t2
        // t3; t0; t1; t2
        for num_absorp in 0..=rhs.len() {
            let lhs_list = rhs[0..num_absorp]
                .iter()
                .enumerate()
                .map(|(i, rhs_conjunction)| {
                    let atoms = if i == 0 {
                        // first => use atoms from lhs conjunction
                        lhs[0].atoms.clone()
                    } else {
                        // not first => new empty list
                        BTreeSet::default()
                    };
                    MatcherConjunction::new(atoms, rhs_conjunction.token.clone())
                })
                .collect_vec();
            let rhs_list = rhs[0..num_absorp].to_vec();
            let mut result_list = lhs_list.into_iter().zip(rhs_list.into_iter()).collect_vec();

            match merge_conjunctions(false, &lhs[1..], &rhs[num_absorp..]) {
                Some(mut recurse_list) => {
                    // got merge
                    result_list.append(&mut recurse_list);
                    return Some(result_list);
                }
                None => continue,
            }
        }
    } else if parent_rhs.is_none() {
        // token_rhs is None => try to absorb 0 to lhs.len() tokens from lhs and recurse with that
        // t3; t0; t1; t2
        // .*;     t1; t2
        for num_absorp in 0..=lhs.len() {
            let lhs_list = lhs[0..num_absorp].to_vec();
            let rhs_list = lhs[0..num_absorp]
                .iter()
                .enumerate()
                .map(|(i, lhs_conjunction)| {
                    let atoms = if i == 0 {
                        // first => use atoms from rhs conjunction
                        rhs[0].atoms.clone()
                    } else {
                        // not first => new empty list
                        BTreeSet::default()
                    };
                    MatcherConjunction::new(atoms, lhs_conjunction.token.clone())
                })
                .collect_vec();
            let mut result_list = lhs_list.into_iter().zip(rhs_list.into_iter()).collect_vec();

            match merge_conjunctions(false, &lhs[num_absorp..], &rhs[1..]) {
                Some(mut recurse_list) => {
                    // got merge
                    result_list.append(&mut recurse_list);
                    return Some(result_list);
                }
                None => continue,
            }
        }
    }

    // no match
    None
}

/// Removes all [MOIRFieldParent::Context] tokens as well as token fields from `lhs` and `rhs` and
/// returns them in a new list with duplicates removed.
fn extract_context(
    lhs: &mut Matcher,
    rhs_maybe: Option<&mut Matcher>,
) -> Vec<MatcherConjunction> {
    // extract lhs context
    let mut lhs_context = lhs
        .conjunctions
        .iter()
        .filter(|conjunction| matches!(conjunction.token.parent, Some(MOIRFieldParent::Context(_))))
        .cloned()
        .collect_vec();

    lhs.conjunctions
        .retain(|conjunction| !matches!(conjunction.token.parent, Some(MOIRFieldParent::Context(_))));

    // extract rhs context and combine with lhs context (if applicable)
    if let Some(rhs) = rhs_maybe {
        // extract lhs context
        let rhs_context = rhs
            .conjunctions
            .iter()
            .filter(|conjunction| matches!(conjunction.token.parent, Some(MOIRFieldParent::Context(_))))
            .cloned()
            .collect_vec();

        rhs.conjunctions
            .retain(|conjunction| !matches!(conjunction.token.parent, Some(MOIRFieldParent::Context(_))));

        // merge context rhs into context lhs
        'next: for mut rhs_conjunction in rhs_context.into_iter() {
            for lhs_conjunction in lhs_context.iter_mut() {
                if rhs_conjunction.token.parent == lhs_conjunction.token.parent {
                    lhs_conjunction.atoms.append(&mut rhs_conjunction.atoms);
                    continue 'next;
                }
            }

            lhs_context.push(rhs_conjunction);
        }
    }

    lhs_context
}
