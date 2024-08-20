use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTConstraintOp,
    SASTConstraintPattern,
};
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::{
    CompilerContext,
    SleighCompiler,
    SleighCompilerResult,
};
use crate::spec::builder::ProcessorSpecificationBuilder;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::field::MOIRFieldId;
use crate::spec::moir::scope::MOIRScope;
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRReference,
    MOIRSlice,
    MOIRSliceEnd,
    MOIRValue,
};
use crate::spec::{
    MOIRSymbol,
    ProcessorSpecificationError,
};
use tracing::trace;

use super::matcher::{
    FieldMatcher,
    FieldMatcherConstraint,
    Matcher,
    MatcherExpression,
    MatcherReference,
};

pub mod atomize;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if it does not exist.
    /// Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        let (constructor, span) = constructor.into_tuple();
        let table_name = constructor
            .table
            .value()
            .map(|sid| self.spec_builder.string(sid))
            .flatten()
            .unwrap_or("instruction");
        trace!("Defining constructor in table {:?}", table_name);
        let mut moir_constructor = MOIRConstructor {
            local_scope: Default::default(),
            instructions: vec![],
        };
        let matcher = self.create_matcher(constructor.bit_pattern_section.into_value().constraint)?;
        self.resolve_symbols_to_local_scope(&mut moir_constructor.local_scope, &matcher.symbols_used)?;

        // TODO: we need to walk the constraint and find out the tokens that are used.
        //       we also need to make sure that there is only one token per section
        //       delimited by a semicolon.

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        let export = self.atomize_pcode(
            &mut moir_constructor.local_scope,
            &mut moir_constructor.instructions,
            constructor.semantic_section.into_value().0,
        )?;
        let table = match constructor.table.lift_option() {
            None => self.spec_builder.modify_root_table()?,
            Some(id) => self.spec_builder.modify_table(id)?,
        };
        if table.exports.is_empty() {
            // If the exports table is empty, we can just insert the export if we have one.
            if let Some(export) = export {
                table.exports.push(export);
            }
        } else {
            // If the exports table already contains exports, we have to make sure that the sizes of all
            // exports match. Also, it is not allowed, that a constructor has no export statement, if
            // another constructor has an export statement. If one constructor exports, all of them have to.
            let Some(export) = export else {
                todo!("Generate error report for constructor missing an export statement")
            };
            let existing_export = table
                .exports
                .first()
                .expect("We just made sure that at least one element exists");
            if export.size() != existing_export.size() {
                todo!("Generate error report for constructor missing an export statement")
            } else {
                // All is well, we can now just insert the export for later type lookup.
                table.exports.push(export);
            }
        }
        Ok(())
    }

    /// Resolve a given list of [MOIRSymbol] used by [Matcher] in its bit pattern section to the
    /// [MOIRScope] of the constructor for usage in the semantic section.
    fn resolve_symbols_to_local_scope(
        &self,
        scope: &mut MOIRScope,
        symbols: &Vec<MOIRSymbol>,
    ) -> SleighCompilerResult {
        trace!("Resolving {} symbols to a local scope", symbols.len());
        for symbol in symbols {
            let value = match symbol {
                MOIRSymbol::Field(field_id) => {
                    let field = self
                        .spec_builder
                        .field_by_symbol_id(*field_id)
                        .ok_or(ProcessorSpecificationError::SymbolUndeclared)?;
                    let num_bytes = (field.bits.clone().count() + 7) / 8;
                    Some(MOIRValue::ValueSlice(MOIRSlice {
                        reference: MOIRReference::Field(*field_id),
                        start: 0,
                        end: MOIRSliceEnd::Value(num_bytes as u8),
                    }))
                }

                MOIRSymbol::Table(table_id) => {
                    let table = self
                        .spec_builder
                        .table_by_symbol_id(*table_id)
                        .ok_or(ProcessorSpecificationError::SymbolUndeclared)?;
                    let export = table.exports.first();
                    if let Some(export) = export {
                        Some(MOIRValue::ValueSlice(MOIRSlice {
                            reference: MOIRReference::SubTable(*table_id),
                            start: 0,
                            end: MOIRSliceEnd::Value(
                                export
                                    .size()
                                    .expect("TODO: Generate error for undetermined size"),
                            ),
                        }))
                    } else {
                        None
                    }
                }

                MOIRSymbol::VarNode(vnid) => {
                    let varnode = self
                        .spec_builder
                        .varnode_by_symbol_id(*vnid)
                        .ok_or(ProcessorSpecificationError::SymbolUndeclared)?;
                    Some(MOIRValue::ValueSlice(MOIRSlice {
                        reference: MOIRReference::VarNode(*vnid),
                        start: 0,
                        end: MOIRSliceEnd::Value(varnode.size as u8),
                    }))
                }

                _ => Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?,
            };

            let ident = self
                .spec_builder
                .ident_by_symbol(symbol)
                .ok_or(ProcessorSpecificationError::SymbolUndeclared)?;
            if let Some(value) = value {
                scope.declare_value(ident.clone().into_value(), value);
            }
        }

        Ok(())
    }

    fn create_matcher(
        &mut self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Matcher> {
        let (constraint, span) = constraint.into_tuple();
        match constraint {
            SASTBitPatternConstraint::Ident(ident) => {
                let symbol = self.spec_builder.get_symbol(&ident.clone().transmute())?;
                let reference = match symbol {
                    MOIRSymbol::Field(field_id) => MatcherReference::Field(*field_id),
                    MOIRSymbol::Table(table_id) => MatcherReference::Subtable(*table_id),
                    MOIRSymbol::VarNode(varnode_id) => MatcherReference::Varnode(*varnode_id),
                    _ => Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?,
                };
                let expression = Box::new(MatcherExpression::Linkage(reference));
                Ok(Matcher::new(expression, vec![*symbol]))
            }
            SASTBitPatternConstraint::BinaryOp(lhs, op, rhs) => {
                // TODO validate: lhs and rhs have to be from the same token
                let mut matcher_lhs = self.create_matcher(*lhs)?;
                let mut matcher_rhs = self.create_matcher(*rhs)?;

                let expression = match op.into_value() {
                    SASTBinaryOp::And => {
                        Box::new(MatcherExpression::And(matcher_lhs.expression, matcher_rhs.expression))
                    }
                    SASTBinaryOp::Or => Box::new(MatcherExpression::Or(matcher_lhs.expression, matcher_rhs.expression)),
                    SASTBinaryOp::Concatenate => Box::new(MatcherExpression::Concat {
                        lhs: matcher_lhs.expression,
                        lhs_tokens: vec![],
                        rhs: matcher_rhs.expression,
                        rhs_tokens: vec![],
                    }),
                };

                matcher_lhs
                    .symbols_used
                    .append(&mut matcher_rhs.symbols_used);

                Ok(Matcher::new(expression, matcher_lhs.symbols_used))
            }
            SASTBitPatternConstraint::UnaryOp(op, val) => {
                // TODO implement
                // todo!("Handle unary op (left-right justify/...)")
                let matcher = self.create_matcher(*val)?;
                Ok(matcher)
            }
            SASTBitPatternConstraint::AdvancedConstraint(lhs, op, pattern) => {
                let lhs_symbol = self.spec_builder.get_symbol(&lhs.clone().transmute())?;

                let expression = match lhs_symbol {
                    MOIRSymbol::Field(field_id) => {
                        self.matcher_from_advanced_constraint(*field_id, op.into_value(), pattern.into_value())
                    }
                    MOIRSymbol::Table(_) => todo!("Integrate subtable into matcher"),
                    _ => Err(ProcessorSpecificationError::InvalidSymbolType(*lhs_symbol))?,
                }?;

                Ok(Matcher::new(expression, vec![*lhs_symbol]))
            }
            SASTBitPatternConstraint::Invalid => {
                todo!("Generate error report for an invalid constraint");
            }
        }
    }

    /// Create a single [MatcherExpression] from a [SASTBitPatternConstraint::AdvancedConstraint]
    /// by splitting advanced constraints like negation and partial order relations into multiple
    /// bit patterns that can be matched against more easily. Be careful since this operation might
    /// create an extremely large matcher expression.
    fn matcher_from_advanced_constraint(
        &self,
        field_id: MOIRFieldId,
        op: SASTConstraintOp,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Box<MatcherExpression>> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or(ProcessorSpecificationError::SymbolUndeclared)?;

        match op {
            SASTConstraintOp::Equals => self.matcher_from_equals_constraint(field_id, pattern),
            SASTConstraintOp::NotEquals => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to negate other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    let bits = field.bits.clone().count() as u16;
                    Ok(self.matcher_from_negate_constraint(field_id, bits, value.into_value()))
                }
            },
            SASTConstraintOp::LessThan => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to implement lt for other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    let bits = field.bits.clone().count() as u16;
                    Ok(self.matcher_from_negate_constraint(field_id, bits, value.into_value()))
                }
            },
            SASTConstraintOp::GreaterThan => match pattern {
                SASTConstraintPattern::Ident(_) => todo!("Not clear how to implement gt for other field"),
                SASTConstraintPattern::BitPattern(value) => {
                    let bits = field.bits.clone().count() as u16;
                    Ok(self.matcher_from_gt_constraint(field_id, bits, value.into_value()))
                }
            },
        }
    }

    // The bit pattern section can contain other relations than equals which have
    // to be broken down by explicitly enumerating all possible values. This
    // operation can be very expensive depending on the size of the field.

    /// Generate a [MatcherExpression] matching on the field being equal to the
    /// pattern.
    fn matcher_from_equals_constraint(
        &self,
        field_id: MOIRFieldId,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Box<MatcherExpression>> {
        let constraint = match pattern {
            SASTConstraintPattern::Ident(ident) => self
                .spec_builder
                .field_id_by_ident(&ident.clone().transmute())
                .map(|field_id| FieldMatcherConstraint::Field(*field_id)),
            SASTConstraintPattern::BitPattern(value) => Ok(FieldMatcherConstraint::BitPattern(value.into_value())),
        }?;

        Ok(Box::new(MatcherExpression::Field(FieldMatcher::new(field_id, constraint))))
    }

    /// Generate an or chain of [MatcherExpression]s matching on every possible
    /// value of field except the provided value.
    fn matcher_from_negate_constraint(
        &self,
        field_id: MOIRFieldId,
        bits: u16,
        value: u64,
    ) -> Box<MatcherExpression> {
        (0u64..2u64.pow(bits as u32))
            .filter_map(|v| {
                if v != value {
                    Some(Box::new(MatcherExpression::Field(FieldMatcher::new(
                        field_id,
                        FieldMatcherConstraint::BitPattern(v),
                    ))))
                } else {
                    None
                }
            })
            .reduce(|acc, expr| Box::new(MatcherExpression::Or(acc, expr)))
            .expect("There should be at least one possible value for which the matcher accepts.")
    }

    /// Generate an or chain of [MatcherExpression]s matching on every possible
    /// value of field except values not less than the provided value.
    fn matcher_from_lt_constraint(
        &self,
        field_id: MOIRFieldId,
        bits: u16,
        value: u64,
    ) -> Box<MatcherExpression> {
        (0u64..2u64.pow(bits as u32))
            .filter_map(|v| {
                if v >= value {
                    Some(Box::new(MatcherExpression::Field(FieldMatcher::new(
                        field_id,
                        FieldMatcherConstraint::BitPattern(v),
                    ))))
                } else {
                    None
                }
            })
            .reduce(|acc, expr| Box::new(MatcherExpression::Or(acc, expr)))
            .expect("There should be at least one possible value for which the matcher accepts.")
    }

    /// Generate an or chain of [MatcherExpression]s matching on every possible
    /// value of field except values not greater than the provided value.
    fn matcher_from_gt_constraint(
        &self,
        field_id: MOIRFieldId,
        bits: u16,
        value: u64,
    ) -> Box<MatcherExpression> {
        (0u64..2u64.pow(bits as u32))
            .filter_map(|v| {
                if v <= value {
                    Some(Box::new(MatcherExpression::Field(FieldMatcher::new(
                        field_id,
                        FieldMatcherConstraint::BitPattern(v),
                    ))))
                } else {
                    None
                }
            })
            .reduce(|acc, expr| Box::new(MatcherExpression::Or(acc, expr)))
            .expect("There should be at least one possible value for which the matcher accepts.")
    }
}
