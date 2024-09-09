use std::collections::HashSet;

use chumsky::container::Container;
use chumsky::primitive::todo;
use chumsky::span::Span;
use num::traits::Pow;
use num::{
    pow,
    PrimInt,
};
use tracing::trace;

use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTConstraintOp,
    SASTConstraintPattern,
    SASTUnaryOp,
};
use crate::compiler::ast::Spanned;
use crate::compiler::build::matcher::{
    FieldMatcher,
    FieldMatcherPattern,
    Matcher,
    MatcherAtom,
    MatcherConjunction,
    MatcherReference,
    MatcherToken,
};
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::MOIRSymbolId;
use crate::spec::moir::field::{
    FieldId,
    MOIRField,
    MOIRFieldParent,
};
use crate::spec::moir::scope::MOIRScope;
use crate::spec::moir::varnode::{
    MOIRReference,
    MOIRSlice,
};
use crate::spec::ProcessorSpecificationError;

impl SleighCompiler {
    /// Create a [Matcher] from a [SASTBitPatternConstraint]. Multiple matchers might be created
    /// due to the splitting of Or operations and the integration of subtables. After the creation,
    /// the following properties hold:
    ///
    /// - Each matcher contains a fixed-length, known list of tokens
    ///     - Each token list has unique context tokens at the beginning
    /// - Each matcher consists only of And statement which are either trivial (only fields being
    /// matched against bit patterns) or nontrivial (including equality of fields)
    pub(super) fn create_matchers(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let (constraint, span) = constraint.into_tuple();
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

    // private helper functions

    fn process_ident(
        &self,
        ident: Spanned<SASTIdent>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        // Ident case: freestanding fields, varnodes or subtables linking global to local symbols. This
        // is one of the base cases and yields multiple simple [Matcher] with [MatcherAtom::Linkage]
        // and the corresponding symbols, fields and tokens.

        let symbol = self.spec_builder.get_symbol(&ident.clone().transmute())?;
        let matchers = match symbol {
            MOIRSymbolId::Table(tid) => {
                // expansion strategy: create new matchers for all possible token
                // lists of submatchers (TODO: is this too expensive?)
                let table = self
                    .spec_builder
                    .table_by_symbol_id(*tid)
                    .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing symbol") })?;
                let token_lists: HashSet<_> = table.matchers.values().map(|m| &m.token_list).collect();
                let matchers = token_lists
                    .into_iter()
                    .map(|tl| {
                        let atoms = vec![MatcherAtom::Linkage(MatcherReference::Subtable(*tid))];
                        let tokens = tl.clone();

                        Matcher::new(vec![MatcherConjunction::new(atoms)], tokens, HashSet::from([*symbol]))
                    })
                    .collect::<Vec<_>>();

                matchers
            }
            MOIRSymbolId::Field(fid) => {
                let field = self
                    .spec_builder
                    .field_by_symbol_id(*fid)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing field mapping")
                    })?;

                let atoms = vec![MatcherAtom::Linkage(MatcherReference::Field(*fid))];
                let tokens = vec![MatcherToken::Determined(field.parent)];

                vec![Matcher::new(
                    vec![MatcherConjunction::new(atoms)],
                    tokens,
                    HashSet::from([*symbol]),
                )]
            }
            MOIRSymbolId::VarNode(vnid) => {
                todo!("Handle varnode reference");
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

        let mut matchers_lhs = self.create_matchers(*lhs)?;
        let mut matchers_rhs = self.create_matchers(*rhs)?;

        let new_matchers = match op.into_value() {
            SASTBinaryOp::And => {
                let mut new_matchers = Vec::new();
                let mut token_lists_seen = HashSet::new();

                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        // make sure tokens are the same (since lhs and rhs can be inconsistent in
                        // case a subtable with multiple constructors was used)
                        let tl_lhs = &matcher_lhs.token_list;
                        let tl_rhs = &matcher_rhs.token_list;
                        if let Ok(tl_longest) = tokens_compatible(tl_lhs, tl_rhs) {
                            // the resulting token lists are not unique so we should
                            // eliminate duplicates
                            // TODO storing the vector twice is unnecessary
                            if !token_lists_seen.insert(tl_longest.to_vec()) {
                                continue;
                            }

                            // this should only be reported as an error if no compatible token
                            // lists exist for all lhs and rhs matchers
                            // TODO: todo!("Generate warning for inconsistent tokens.");

                            let conjunctions = matcher_lhs
                                .conjunctions
                                .iter()
                                .zip(matcher_rhs.conjunctions.iter())
                                .map(|(c_lhs, c_rhs)| {
                                    let mut merged_atoms = c_lhs.matcher_atoms.clone();
                                    merged_atoms.append(&mut c_rhs.matcher_atoms.clone());
                                    MatcherConjunction::new(merged_atoms)
                                })
                                .collect::<Vec<_>>();

                            let token_list = tl_longest.to_vec();
                            let mut symbols_used = matcher_lhs.symbols_used.clone();
                            symbols_used.extend(matcher_rhs.symbols_used.clone());

                            new_matchers.push(Matcher::new(conjunctions, token_list, symbols_used));
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
                let tl_lhs = &matchers_lhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                let tl_rhs = &matchers_rhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;

                let token_list = tokens_compatible(tl_lhs, tl_rhs)?.to_vec();
                matchers_lhs.iter_mut().for_each(|matcher| {
                    matcher.token_list = token_list.clone();
                });

                matchers_rhs.iter_mut().for_each(|matcher| {
                    matcher.token_list = token_list.clone();
                });

                // combining them is done by union
                matchers_lhs.append(&mut matchers_rhs);
                matchers_lhs
            }
            SASTBinaryOp::Concatenate => {
                // same as And but we concatenate the token lists and conjunctions
                // instead of merging them
                let tl_lhs = &matchers_lhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                let tl_rhs = &matchers_rhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;

                let mut new_matchers = Vec::new();
                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        // TODO merge justify
                        let mut conjunctions = matcher_lhs.conjunctions.clone();
                        conjunctions.append(&mut matcher_rhs.conjunctions.clone());
                        let mut token_list = matcher_lhs.token_list.clone(); // just the same
                        token_list.append(&mut matcher_rhs.token_list.clone());
                        let mut symbols_used = matcher_lhs.symbols_used.clone();
                        symbols_used.extend(matcher_rhs.symbols_used.clone());

                        let cleaned_tokens = clean_context(token_list);
                        new_matchers.push(Matcher::new(conjunctions, cleaned_tokens, symbols_used));
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
        let mut matchers = self.create_matchers(*operand)?;
        // append/prepend undetermined token to each matcher
        match op.into_value() {
            SASTUnaryOp::JustifyLeft => {
                for matcher in matchers.iter_mut() {
                    matcher.token_list.push(MatcherToken::Undetermined);
                }
            }
            SASTUnaryOp::JustifyRight => {
                for matcher in matchers.iter_mut() {
                    let mut new_tokens = Vec::with_capacity(matcher.token_list.len() + 1);
                    new_tokens.push(MatcherToken::Undetermined);
                    new_tokens.append(&mut matcher.token_list);
                    matcher.token_list = new_tokens;
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
        let lhs_symbol = self.spec_builder.get_symbol(&lhs.clone().transmute())?;

        let (conjunctions, parent) = match lhs_symbol {
            MOIRSymbolId::Field(field_id) => {
                let conjunctions = self.matcher_conjunctions_from_advanced_constraint(
                    *field_id,
                    op.into_value(),
                    pattern.into_value(),
                )?;
                let field = self
                    .spec_builder
                    .field_by_symbol_id(*field_id)
                    .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing field") })?;
                Ok::<_, SleighCompilerError>((conjunctions, MatcherToken::Determined(field.parent)))
            }
            MOIRSymbolId::Table(_) => todo!("Integrate subtable into matcher"),
            _ => Err(ProcessorSpecificationError::InvalidSymbolType(*lhs_symbol))?,
        }?;

        let token_list = vec![parent];

        // one matcher per conjunction returned
        Ok(conjunctions
            .into_iter()
            .map(|c| Matcher::new(vec![c], token_list.clone(), HashSet::from([*lhs_symbol])))
            .collect::<Vec<_>>())
    }

    /// Create multiple [MatcherConjunction] from a [SASTBitPatternConstraint::AdvancedConstraint]
    /// by splitting advanced constraints like negation and partial order relations into multiple
    /// bit patterns that can be matched against more easily. Be careful since this operation might
    /// create an extremely large matcher conjunction.
    fn matcher_conjunctions_from_advanced_constraint(
        &self,
        field_id: FieldId,
        op: SASTConstraintOp,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Vec<MatcherConjunction>> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for symbol undeclared") })?;

        match pattern {
            SASTConstraintPattern::Ident(ident) => {
                let lhs_field_id = self.spec_builder.field_id_by_ident(&ident.transmute())?;
                let lhs_field = self
                    .spec_builder
                    .field_by_symbol_id(*lhs_field_id)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for wrong symbol type or missing symbol")
                    })?;
                if lhs_field.bits.size() != field.bits.size() {
                    todo!("Generate error report for mismatching field sizes");
                }

                let pattern = match op {
                    SASTConstraintOp::Equals => FieldMatcherPattern::Equals(*lhs_field_id),
                    SASTConstraintOp::NotEquals => FieldMatcherPattern::NotEquals(*lhs_field_id),
                    SASTConstraintOp::LessThan => FieldMatcherPattern::LessThan(*lhs_field_id),
                    SASTConstraintOp::GreaterThan => FieldMatcherPattern::GreaterThan(*lhs_field_id),
                };

                let field_matcher = FieldMatcher::new(field_id, pattern);
                Ok(vec![MatcherConjunction::new(vec![MatcherAtom::Field(field_matcher)])])
            }
            SASTConstraintPattern::BitPattern(value) => {
                let conjunctions = match op {
                    SASTConstraintOp::Equals => {
                        vec![self.matcher_conjunction_from_equals_constraint(field_id, field, value.into_value())]
                    }
                    SASTConstraintOp::NotEquals => {
                        self.matcher_conjunction_from_negate_constraint(field_id, field, value.into_value())
                    }
                    SASTConstraintOp::LessThan => {
                        self.matcher_conjunction_from_lt_constraint(field_id, field, value.into_value())
                    }
                    SASTConstraintOp::GreaterThan => {
                        self.matcher_conjunction_from_gt_constraint(field_id, field, value.into_value())
                    }
                };

                Ok(conjunctions)
            }
        }
    }

    // The bit pattern section can contain other relations than equals which have
    // to be broken down by explicitly enumerating all possible values. This
    // operation can be very expensive depending on the size of the field.

    /// Generate a [MatcherConjunction] matching on the field being equal to the value.
    fn matcher_conjunction_from_equals_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> MatcherConjunction {
        let pattern = FieldMatcherPattern::BitPattern {
            value,
            shift: field.bits.start,
            mask: 2u64.pow(field.bits.size() as u32) - 1,
        };

        let atom = MatcherAtom::Field(FieldMatcher::new(field_id, pattern));

        MatcherConjunction::new(vec![atom])
    }

    /// Generate multiple [MatcherExpression] matching on every possible
    /// value of field except the provided value.
    fn matcher_conjunction_from_negate_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<MatcherConjunction> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v != value {
                    Some(self.matcher_conjunction_from_equals_constraint(field_id, field, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Generate multiple [MatcherExpression] matching on every possible
    /// value of field except values not less than the provided value.
    fn matcher_conjunction_from_lt_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<MatcherConjunction> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v < value {
                    Some(self.matcher_conjunction_from_equals_constraint(field_id, field, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    /// Generate an or chain of [MatcherExpression]s matching on every possible
    /// value of field except values not greater than the provided value.
    fn matcher_conjunction_from_gt_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<MatcherConjunction> {
        (0u64..2u64.pow(field.bits.size() as u32))
            .filter_map(|v| {
                if v > value {
                    Some(self.matcher_conjunction_from_equals_constraint(field_id, field, v))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}

/// Checks whether two token lists are compatible with one another, i.e. if they are equal taking
/// potential left/right justification for different lengths into account.

/// # Return
/// - Ok(list) if the lists are compatible with `list` being the more specific (i.e. longer) one
/// - Err() if the lists are not compatible
fn tokens_compatible<'a>(
    lhs: &'a [MatcherToken],
    rhs: &'a [MatcherToken],
) -> SleighCompilerResult<&'a [MatcherToken]> {
    if lhs.len() == 0 && rhs.len() == 0 {
        return Ok(&lhs);
    } else if (lhs.len() == 0 && rhs.len() != 0) || (lhs.len() != 0 && rhs.len() == 0) {
        Err(SleighCompilerError::TODO)?;
    }

    let lhs_padded_start = matches!(lhs.first().expect("We just checked this"), MatcherToken::Undetermined);
    let lhs_padded_end = matches!(lhs.last().expect("We just checked this"), MatcherToken::Undetermined);
    let rhs_padded_start = matches!(rhs.first().expect("We just checked this"), MatcherToken::Undetermined);
    let rhs_padded_end = matches!(rhs.last().expect("We just checked this"), MatcherToken::Undetermined);

    if lhs_padded_start && rhs_padded_start {
        // find most common suffix
        let i = lhs
            .iter()
            .zip(rhs.iter())
            .rev()
            .take_while(|(l, r)| l == r && !matches!(l, MatcherToken::Undetermined))
            .count();
        return Ok(&lhs[(lhs.len() + 1 - i)..]);
    }

    if lhs_padded_end && rhs_padded_end {
        // find most common prefix
        let i = lhs
            .iter()
            .zip(rhs.iter())
            .take_while(|(l, r)| l == r && !matches!(l, MatcherToken::Undetermined))
            .count();
        return Ok(&lhs[0..i]);
    }

    // padded one contained in longer? (start/end/middle)
    let (padded, non_padded, pad_left, pad_right) = if lhs_padded_start || lhs_padded_end {
        (lhs, rhs, lhs_padded_start, lhs_padded_end)
    } else {
        (rhs, lhs, rhs_padded_start, rhs_padded_end)
    };

    if padded.len() > non_padded.len() {
        Err(SleighCompilerError::TODO)?;
    }

    if pad_left && pad_right {
        // both padded => is padded list contained in non_padded? (excluding Undetermined)
        for si in 0..non_padded.len() {
            if &padded[1..(padded.len() - 1)] == &non_padded[si..(si + padded.len() - 2)] {
                return Ok(non_padded);
            }
        }
    } else if pad_left {
        // start padded => is smaller list present at end of larger? (excluding Undetermined)
        if &padded[1..(padded.len())] == &non_padded[(non_padded.len() + 1 - padded.len())..] {
            return Ok(non_padded);
        }
    } else if pad_right {
        // end padded => is smaller list present at start of larger? (excluding Undetermined)
        if &padded[..(padded.len() - 1)] == &non_padded[..(padded.len() - 1)] {
            return Ok(non_padded);
        }
    }

    // nothing padded => regular full-length search
    if padded == non_padded {
        Ok(non_padded)
    } else {
        Err(SleighCompilerError::TODO)
    }
}

/// Cleans a list by removing duplicate [MOIRFieldParent::Context] references and
/// moving the remaining ones to the front
fn clean_context(token_list: Vec<MatcherToken>) -> Vec<MatcherToken> {
    let mut context = token_list
        .clone()
        .into_iter()
        .filter(|token| {
            if let MatcherToken::Determined(d) = token {
                matches!(d, MOIRFieldParent::Context(_))
            } else {
                false
            }
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    context.extend(token_list.into_iter().filter(|token| {
        if let MatcherToken::Determined(d) = token {
            !matches!(d, MOIRFieldParent::Context(_))
        } else {
            false
        }
    }));

    context
}

/// Checks whether two tokens are compatible with one another. This is the
/// case iff one of them is of type [MatcherToken::Determined(MOIRFieldParent::Context)]
/// of if they are equal.
fn token_compatible(
    lhs: &MatcherToken,
    rhs: &MatcherToken,
) -> bool {
    lhs == rhs
        || matches!(lhs, MatcherToken::Determined(MOIRFieldParent::Context(_)))
        || matches!(rhs, MatcherToken::Determined(MOIRFieldParent::Context(_)))
}
