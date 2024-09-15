use std::collections::HashSet;

use chumsky::container::Container;

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
        let new_matchers = self.create_matchers_recursive(constraint);

        // TODO remove trailing Undetermined (or not, idk yet)

        new_matchers
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
        let matchers = match symbol {
            MOIRSymbolId::Table(tid) => {
                // expansion strategy: hoist matchers of subtable up (this might be too expensive
                // but probably isn't since most matchers are removed when merging token lists)
                let table = self
                    .spec_builder
                    .table_by_symbol_id(*tid)
                    .ok_or_else(|| -> SleighCompilerError { todo!("Generate error report for missing symbol") })?;

                table.matchers.values().cloned().collect::<Vec<_>>()
            }
            MOIRSymbolId::Field(fid) => {
                let field = self
                    .spec_builder
                    .field_by_symbol_id(*fid)
                    .ok_or_else(|| -> SleighCompilerError {
                        todo!("Generate error report for missing field mapping")
                    })?;

                let atom = MatcherAtom::Linkage(MatcherReference::Field(*fid));
                let tokens = vec![MatcherToken::Determined(field.parent)];

                vec![Matcher::new(
                    vec![HashSet::from([atom])],
                    tokens,
                    vec![HashSet::from([*symbol])],
                )]
            }
            MOIRSymbolId::VarNode(vnid) => {
                let atom = MatcherAtom::Linkage(MatcherReference::Varnode(*vnid));
                let tokens = vec![MatcherToken::Undetermined]; // TODO is this correct?

                vec![Matcher::new(
                    vec![HashSet::from([atom])],
                    tokens,
                    vec![HashSet::from([*symbol])],
                )]
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
        // eprintln!("matchers_lhs = {:#?}", matchers_lhs);
        // eprintln!("matchers_rhs = {:#?}", matchers_rhs);

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
                        if let Ok(tl_longest) =
                            merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list)
                        {
                            let mut token_list = context_matcher.token_list;
                            token_list.append(&mut tl_longest.to_vec());

                            // this should only be reported as an error if no compatible token
                            // lists exist for all lhs and rhs matchers
                            // TODO: todo!("Generate warning for inconsistent tokens.");

                            let mut conjunctions = context_matcher.conjunctions;
                            conjunctions.append(&mut merge_conjunctions(
                                &matcher_lhs_nc.conjunctions,
                                &matcher_rhs_nc.conjunctions,
                            ));

                            let mut symbols_used = context_matcher.symbols_used;
                            symbols_used.append(&mut merge_symbols_used(
                                &matcher_lhs_nc.symbols_used,
                                &matcher_rhs_nc.symbols_used,
                            ));

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

                let mut merged_tokens = merge_token_lists(&matcher_lhs_nc.token_list, &matcher_rhs_nc.token_list)?.to_vec();
                matchers_lhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_lhs.token_list.clone();
                    token_list.append(&mut merged_tokens.clone());
                    // Add missing conjunction/symbol lists in case the resulting token list is
                    // longer than the original list. This is not necessary for the other cases
                    // since everything is merged into one which we do not do here.
                    for _ in matcher.token_list.len()..token_list.len() {
                        matcher.symbols_used.push(HashSet::new());
                        matcher.conjunctions.push(HashSet::new());
                    }
                    matcher.token_list = token_list;
                });

                matchers_rhs.iter_mut().for_each(|matcher| {
                    let mut token_list = context_rhs.token_list.clone();
                    token_list.append(&mut merged_tokens);
                    for _ in matcher.token_list.len()..token_list.len() {
                        matcher.symbols_used.push(HashSet::new());
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
                        let mut symbols_used = context_matcher.symbols_used.clone();
                        symbols_used.append(&mut matcher_lhs_nc.symbols_used.clone());
                        symbols_used.append(&mut matcher_rhs_nc.symbols_used.clone());
                        let mut token_list = context_matcher.token_list.clone();
                        token_list.append(&mut matcher_lhs_nc.token_list.clone());
                        token_list.append(&mut matcher_rhs_nc.token_list.clone());

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
            .map(|c| Matcher::new(vec![c], token_list.clone(), vec![HashSet::from([*lhs_symbol])]))
            .collect::<Vec<_>>())
    }

    /// Create multiple conjunctions from a [SASTBitPatternConstraint::AdvancedConstraint]
    /// by splitting advanced constraints like negation and partial order relations into multiple
    /// bit patterns that can be matched against more easily. Be careful since this operation might
    /// create an extremely large matcher conjunction.
    fn matcher_conjunctions_from_advanced_constraint(
        &self,
        field_id: FieldId,
        op: SASTConstraintOp,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Vec<HashSet<MatcherAtom>>> {
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
                Ok(vec![HashSet::from([MatcherAtom::Field(field_matcher)])])
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

    /// Generate a matcher conjunction matching on the field being equal to the value.
    fn matcher_conjunction_from_equals_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> HashSet<MatcherAtom> {
        let pattern = FieldMatcherPattern::Value(value);

        let atom = MatcherAtom::Field(FieldMatcher::new(field_id, pattern));

        HashSet::from([atom])
    }

    /// Generate multiple matcher conjunctions matching on every possible
    /// value of field except the provided value.
    fn matcher_conjunction_from_negate_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
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

    /// Generate multiple matcher atoms matching on every possible
    /// value of field except values not less than the provided value.
    fn matcher_conjunction_from_lt_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
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

    /// Generate an or chain of matcher atoms matching on every possible
    /// value of field except values not greater than the provided value.
    fn matcher_conjunction_from_gt_constraint(
        &self,
        field_id: FieldId,
        field: &MOIRField,
        value: u64,
    ) -> Vec<HashSet<MatcherAtom>> {
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

/// Merge two token lists by checking whether they are compatible with one another, i.e. if they
/// are equal taking potential left/right justification for different lengths into account. Two
/// lists are always compatible if one of them is empty.

/// # Return
/// - Ok(list) if the lists are compatible with `list` being the more specific (i.e. longer) one
/// - Err() if the lists are not compatible
fn merge_token_lists<'a>(
    lhs: &'a [MatcherToken],
    rhs: &'a [MatcherToken],
) -> SleighCompilerResult<&'a [MatcherToken]> {
    if lhs.len() == 0 {
        return Ok(&rhs);
    } else if rhs.len() == 0 {
        return Ok(&lhs);
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

/// Removes all [MOIRFieldParent::Context] tokens as well as symbol lists from `lhs` and `rhs` and
/// returns them in a new list with duplicates removed.
fn extract_context(
    lhs: &mut Matcher,
    rhs_maybe: Option<&mut Matcher>,
) -> Matcher {
    let context_lhs = lhs
        .token_list
        .iter()
        .zip(lhs.symbols_used.iter())
        .zip(lhs.conjunctions.iter())
        .filter_map(|((token, symbols), conjunction)| {
            if let MatcherToken::Determined(d) = token {
                if matches!(d, MOIRFieldParent::Context(_)) {
                    return Some(((token, symbols), conjunction));
                }
            }

            None
        });

    let mut context_rhs = if let Some(ref rhs) = rhs_maybe {
        rhs.token_list
            .iter()
            .zip(rhs.symbols_used.iter())
            .zip(rhs.conjunctions.iter())
            .filter_map(|((token, symbols), conjunction)| {
                if let MatcherToken::Determined(d) = token {
                    if matches!(d, MOIRFieldParent::Context(_)) {
                        return Some(((token.clone(), symbols.clone()), conjunction.clone()));
                    }
                }

                None
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // merge context_lhs into context_rhs and create context matcher
    'next: for ((token, symbols), conjunctions) in context_lhs {
        for ((token_rhs, symbols_rhs), conjunctions_rhs) in context_rhs.iter_mut() {
            if token == token_rhs {
                symbols_rhs.extend(symbols);
                conjunctions_rhs.extend(conjunctions);
                break 'next;
            }
        }

        context_rhs.push(((token.clone(), symbols.clone()), conjunctions.clone()));
    }

    let ((context_tokens, context_symbols), context_conjunctions): ((Vec<_>, Vec<_>), Vec<_>) =
        context_rhs.into_iter().unzip();
    let context_matcher = Matcher::new(context_conjunctions, context_tokens, context_symbols);

    let lhs_new = lhs
        .token_list
        .iter()
        .zip(lhs.symbols_used.iter())
        .zip(lhs.conjunctions.iter())
        .filter_map(|((token, symbols), conjunction)| {
            if let MatcherToken::Determined(d) = token {
                if matches!(d, MOIRFieldParent::Context(_)) {
                    return None;
                }
            }

            Some(((token.clone(), symbols.clone()), conjunction.clone()))
        });

    let ((lhs_tokens, lhs_symbols), lhs_conjunctions): ((Vec<_>, Vec<_>), Vec<_>) = lhs_new.into_iter().unzip();
    *lhs = Matcher::new(lhs_conjunctions, lhs_tokens, lhs_symbols);

    if let Some(rhs) = rhs_maybe {
        let rhs_new = rhs
            .token_list
            .iter()
            .zip(rhs.symbols_used.iter())
            .zip(rhs.conjunctions.iter())
            .filter_map(|((token, symbols), conjunction)| {
                if let MatcherToken::Determined(d) = token {
                    if matches!(d, MOIRFieldParent::Context(_)) {
                        return None;
                    }
                }

                Some(((token.clone(), symbols.clone()), conjunction.clone()))
            });

        let ((rhs_tokens, rhs_symbols), rhs_conjunctions): ((Vec<_>, Vec<_>), Vec<_>) = rhs_new.into_iter().unzip();
        *rhs = Matcher::new(rhs_conjunctions, rhs_tokens, rhs_symbols);
    }

    context_matcher
}

/// Merge two symbol lists by combining sets at identical positions through union and extending
/// the smaller list with elements from the larger one.
fn merge_symbols_used(
    lhs: &Vec<HashSet<MOIRSymbolId>>,
    rhs: &Vec<HashSet<MOIRSymbolId>>,
) -> Vec<HashSet<MOIRSymbolId>> {
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
            conjunctions.extend(conjunctions_rhs);
        } else {
            conjunctions_combined.push(conjunctions_rhs.clone());
        }
    }

    conjunctions_combined
}
