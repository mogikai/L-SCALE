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
    SASTBinaryOp, SASTBitPatternConstraint, SASTConstraintOp, SASTConstraintPattern, SASTUnaryOp
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
};
use crate::spec::moir::scope::MOIRScope;
use crate::spec::moir::varnode::{
    MOIRReference,
    MOIRSlice,
};
use crate::spec::ProcessorSpecificationError;

impl SleighCompiler {
    /// Create a [Matcher] from a [SASTBitPatternConstraint]. Multiple matchers might
    /// be created due to the splitting of Or operations. The vector will never be
    /// empty on successful return.
    pub(super) fn create_matchers(
        &self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let (constraint, span) = constraint.into_tuple();
        match constraint {
            SASTBitPatternConstraint::Ident(ident) => {
                // Ident case: freestanding fields, varnodes or subtables linking global to local symbols. This
                // is one of the base cases and yields a simple [Matcher] with [MatcherAtom::Linkage]
                // and the corresponding symbols and fields.

                let symbol = self.spec_builder.get_symbol(&ident.clone().transmute())?;
                let (reference, tokens) = match symbol {
                    MOIRSymbolId::Table(tid) => {
                        // table: pull out all matchers and substitute
                        let tokens = Vec::new();
                        (MatcherReference::Subtable(*tid), tokens)
                    }
                    MOIRSymbolId::Field(fid) => {
                        let field =
                            self.spec_builder
                            .field_by_symbol_id(*fid)
                            .ok_or_else(|| -> SleighCompilerError {
                                todo!("Generate error report for missing field mapping")
                            })?;
                        (MatcherReference::Field(*fid), vec![MatcherToken::Determined(field.parent)])
                    }
                    MOIRSymbolId::VarNode(vnid) => {
                        let tokens = Vec::new(); // is a varnode even contained in a token? TODO: remove (probably)
                        (MatcherReference::Varnode(*vnid), tokens)
                    }
                    _ => todo!("Generate error report for invalid symbol type"),
                };

                let conjunction = MatcherConjunction::new(vec![MatcherAtom::Linkage(reference)]);
                Ok(vec![Matcher::new(vec![conjunction], tokens, vec![*symbol])])
            }
            SASTBitPatternConstraint::BinaryOp(lhs, op, rhs) => {
                let new_matchers = self.process_binary_op(lhs, op, rhs);
                new_matchers
            }
            SASTBitPatternConstraint::UnaryOp(op, operand) => {
                let new_matchers = self.process_unary_op(op, operand);
                new_matchers
            }
            SASTBitPatternConstraint::AdvancedConstraint(lhs, op, pattern) => {
                let new_matchers = self.process_advanced_constraint(lhs, op, pattern);
                new_matchers
            }
            SASTBitPatternConstraint::Invalid => {
                todo!("Generate error report for an invalid constraint");
            }
        }
    }

    // private helper functions

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
                // make sure tokens are the same (since lhs and rhs are consistent
                // themselves, it's enough to check the first elements)
                let tl_lhs = &matchers_lhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                let tl_rhs = &matchers_rhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                if !tokens_compatible(tl_lhs, tl_rhs) {
                    todo!("Generate error report for inconsistent tokens.");
                }

                let mut new_matchers = Vec::new();
                // combining them has to be done as a Cartesian product
                for matcher_lhs in &matchers_lhs {
                    for matcher_rhs in &matchers_rhs {
                        // TODO merge justify
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

                        let token_list = matcher_lhs.token_list.clone(); // just the same
                        let mut symbols_used = matcher_lhs.symbols_used.clone();
                        symbols_used.append(&mut matcher_rhs.symbols_used.clone());

                        new_matchers.push(Matcher::new(conjunctions, token_list, symbols_used));
                    }
                }

                new_matchers
            }
            SASTBinaryOp::Or => {
                // make sure tokens are the same (since lhs and rhs are consistent
                // themselves, it's enough to check the first elements) taking
                // left/right justification into account (tl.lhs.len() !+ tl_rhs.len())
                let tl_lhs = &matchers_lhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                let tl_rhs = &matchers_rhs
                    .first()
                    .expect("A matcher has to exist on successful return")
                    .token_list;
                if !tokens_compatible(tl_lhs, tl_rhs) {
                    todo!("Generate error report for inconsistent tokens.");
                }

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
                        symbols_used.append(&mut matcher_rhs.symbols_used.clone());

                        new_matchers.push(Matcher::new(conjunctions, token_list, symbols_used));
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
        operand: Box<Spanned<SASTBitPatternConstraint>>
    ) -> SleighCompilerResult<Vec<Matcher>> {
        let mut matchers = self.create_matchers(*operand)?;
        // append/prepend undetermined token to each matcher
        match op.into_value() {
            SASTUnaryOp::JustifyLeft => {
                for matcher in matchers.iter_mut() {
                    matcher.token_list.push(MatcherToken::Undetermined);
                }
            },
            SASTUnaryOp::JustifyRight => {
                for matcher in matchers.iter_mut() {
                    let mut new_tokens = Vec::with_capacity(matcher.token_list.len() + 1);
                    new_tokens.push(MatcherToken::Undetermined);
                    new_tokens.append(&mut matcher.token_list);
                    matcher.token_list = new_tokens;
                }
            },
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

        let (conjunctions, parent) =
            match lhs_symbol {
                MOIRSymbolId::Field(field_id) => {
                    let conjunctions = self.matcher_conjunctions_from_advanced_constraint(
                        *field_id,
                        op.into_value(),
                        pattern.into_value(),
                    )?;
                    let field = self.spec_builder.field_by_symbol_id(*field_id).ok_or_else(
                        || -> SleighCompilerError { todo!("Generate error report for missing field") },
                    )?;
                    Ok::<_, SleighCompilerError>((conjunctions, MatcherToken::Determined(field.parent)))
                }
                MOIRSymbolId::Table(_) => todo!("Integrate subtable into matcher"),
                _ => Err(ProcessorSpecificationError::InvalidSymbolType(*lhs_symbol))?,
            }?;

        let token_list = vec![parent];

        // one matcher per conjunction returned
        Ok(conjunctions
            .into_iter()
            .map(|c| Matcher::new(vec![c], token_list.clone(), vec![*lhs_symbol]))
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

        match op {
            SASTConstraintOp::Equals => match pattern {
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

                    let field_matcher = FieldMatcher::new(field_id, FieldMatcherPattern::Field(*lhs_field_id));
                    Ok(vec![MatcherConjunction::new(vec![MatcherAtom::Field(field_matcher)])])
                }
                SASTConstraintPattern::BitPattern(value) => {
                    Ok(vec![self.matcher_conjunction_from_equals_constraint(field_id, field, value.into_value())])
                }
            },
            SASTConstraintOp::NotEquals => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to negate other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    Ok(self.matcher_conjunction_from_negate_constraint(field_id, field, value.into_value()))
                }
            },
            SASTConstraintOp::LessThan => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to implement lt for other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    Ok(self.matcher_conjunction_from_lt_constraint(field_id, field, value.into_value()))
                }
            },
            SASTConstraintOp::GreaterThan => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to implement gt for other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    Ok(self.matcher_conjunction_from_gt_constraint(field_id, field, value.into_value()))
                }
            },
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

/// Tests whether two token lists are compatible with one another, i.e. if they are equal taking
/// potential left/right justification for different lengths into account.
fn tokens_compatible(
    lhs: &Vec<MatcherToken>,
    rhs: &Vec<MatcherToken>,
) -> bool {
    let (short, long) = if lhs.len() < rhs.len() { (lhs, rhs) } else { (rhs, lhs) };
    if short.len() == 0 {
        return long.len() == 0;
    }

    let padded_start = matches!(lhs.first().expect("We just checked this"), MatcherToken::Undetermined);
    let padded_end = matches!(lhs.last().expect("We just checked this"), MatcherToken::Undetermined);

    if padded_start && padded_end {
        // both padded => is smaller list contained in larger?
        for si in 0..(long.len() - short.len() + 1) {
            if &short[..] == &long[si..] {
                return true;
            }
        }
    } else if padded_start {
        // start padded => is smaller list present at end of larger?
        if &short[..] == &long[long.len() - short.len()..] {
            return true;
        }
    } else if padded_end {
        // end padded => is smaller list present at start of larger?
        if &short[..] == &long[..short.len()] {
            return true;
        }
    }

    // nothing padded => regular full-length search
    short == long
}
