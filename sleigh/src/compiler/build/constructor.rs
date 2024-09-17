use super::matcher::{
    FieldMatcher,
    FieldMatcherConstraint,
    Matcher,
    MatcherExpression,
    MatcherReference,
};
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
use crate::spec::builder::{
    MOIRSymbolId,
    StringId,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::{
    MOIRReference,
    MOIRSlice,
};
use crate::spec::ProcessorSpecificationError;
use tracing::{
    debug,
    trace,
};

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if it does not exist.
    /// Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
        let (constructor, _) = constructor.into_tuple();
        let table_name = constructor
            .table
            .value()
            .map(|sid| self.spec_builder.string(sid))
            .flatten()
            .unwrap_or("instruction");
        trace!("Defining constructor in table {:?}", table_name);
        debug!("Instruction: {}", constructor.display_section.display_str.value());

        // TODO: Need to parse the disassembly section here as well and add that information to the matcher
        let matcher = self.create_matcher(constructor.bit_pattern_section.into_value().constraint)?;

        let scope =
            self.pc_create_scope(constructor.display_section.into_value().identifiers, &matcher.symbols_used)?;

        let mut moir_constructor = MOIRConstructor {
            local_scope: scope,
            instructions: vec![],
        };

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
        self.pc_handle_export(constructor.table.transpose(), export)?;
        let _id = self.spec_builder.define_constructor(moir_constructor)?;
        // TODO: Insert the constructor id into the corresponding matcher and/or the table.
        Ok(())
    }

    fn pc_handle_export(
        &mut self,
        table: Option<Spanned<StringId>>,
        export: Option<MOIRExport>,
    ) -> Result<(), SleighCompilerError> {
        let table = match table {
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
                todo!("Generate error report for inconsistent sizes of exports")
            } else {
                // All is well, we can now just insert the export for later type lookup.
                table.exports.push(export);
            }
        }
        Ok(())
    }

    fn pc_create_scope(
        &mut self,
        display_identifiers: Vec<Spanned<StringId>>,
        matcher_symbols: &[MOIRSymbolId],
    ) -> Result<MOIRScope, SleighCompilerError> {
        let mut scope = MOIRScope::default();
        // Declare the special identifiers that are filled with constants upon calling the constructor.
        // Their sizes are undetermined and inferred from context.
        scope.declare_local(self.spec_builder.intern("inst_start"), MOIRSlice::inst_start());
        scope.declare_local(self.spec_builder.intern("inst_next"), MOIRSlice::inst_next());
        scope.declare_local(self.spec_builder.intern("inst_next2"), MOIRSlice::inst_next2());

        self.resolve_symbols_to_local_scope(&mut scope, matcher_symbols)?;
        for ident in display_identifiers {
            // TODO: We probably need to figure out a better way to handle these locals that are constructed
            //       in the disassembly section.
            let local = scope.local(*ident.value());
            if local.is_none() {
                let str = self.spec_builder.resolve_string_id(*ident.value());
                trace!("Local scope is missing identifier {str}. Allocating it now.");
                scope.allocate_named_temporary(ident.clone(), MOIRMaybeSize::Undetermined)?;
            }
        }
        Ok(scope)
    }

    /// Resolve a given list of [MOIRSymbolId] used by [Matcher] in its bit pattern section to the
    /// [MOIRScope] of the constructor for usage in the semantic section.
    fn resolve_symbols_to_local_scope(
        &self,
        scope: &mut MOIRScope,
        symbols: &[MOIRSymbolId],
    ) -> SleighCompilerResult {
        trace!("Resolving {} symbols to a local scope", symbols.len());
        for symbol in symbols {
            let value = match symbol {
                MOIRSymbolId::Field(field_id) => {
                    let field = self
                        .spec_builder
                        .field_by_symbol_id(*field_id)
                        .ok_or(SleighCompilerError::SymbolUndeclared)?;
                    let size = if !field.variables.is_empty() {
                        // The field has variables attached. These will be later given to the constructor during
                        // decoding. We can now set the size of the value that is referenced to the size of the
                        // attached varnode.
                        let vnid = *field
                            .variables
                            .first()
                            .expect("We just made sure there is at least one element");
                        self.spec_builder
                            .varnode_by_symbol_id(vnid)
                            .expect("This must exist. Otherwise we have a programming mistake")
                            .size
                            .into()
                    } else {
                        // TODO: Check if values are attached and infer the maximum size from them.
                        // TODO: We cannot pass a size here since then the sleigh spec breaks
                        //       because they cannot decide whether to explicitly enforce sizes
                        //       or to just implicitly extend sizes because reasons.......
                        // (((field.bits.clone().count() + 7) / 8) as MOIRSize).into()
                        MOIRMaybeSize::Undetermined
                    };
                    Some(MOIRSlice {
                        reference: MOIRReference::Field(*field_id),
                        start: 0,
                        end: size,
                    })
                }

                MOIRSymbolId::Table(table_id) => {
                    let table = self
                        .spec_builder
                        .table_by_symbol_id(*table_id)
                        .ok_or(SleighCompilerError::SymbolUndeclared)?;
                    let export = table.exports.first();
                    if let Some(export) = export {
                        Some(MOIRSlice {
                            reference: MOIRReference::SubTable(*table_id),
                            start: 0,
                            end: export.size(),
                        })
                    } else {
                        None
                    }
                }

                MOIRSymbolId::VarNode(vnid) => {
                    let varnode = self
                        .spec_builder
                        .varnode_by_symbol_id(*vnid)
                        .ok_or(SleighCompilerError::SymbolUndeclared)?;
                    Some(MOIRSlice {
                        reference: MOIRReference::VarNode(*vnid),
                        start: 0,
                        end: MOIRMaybeSize::Determined(varnode.size),
                    })
                }

                _ => Err(ProcessorSpecificationError::InvalidSymbolType(*symbol))?,
            };

            let ident = self
                .spec_builder
                .ident_by_symbol(symbol)
                .ok_or(SleighCompilerError::SymbolUndeclared)?;
            if let Some(value) = value {
                scope.declare_local(ident.clone().into_value(), value);
            }
        }

        Ok(())
    }

    fn create_matcher(
        &mut self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> SleighCompilerResult<Matcher> {
        let (constraint, _) = constraint.into_tuple();
        match constraint {
            SASTBitPatternConstraint::Ident(ident) => {
                let symbol = self.spec_builder.get_symbol(&ident.clone().transmute())?;
                let reference = match symbol {
                    MOIRSymbolId::Field(field_id) => MatcherReference::Field(*field_id),
                    MOIRSymbolId::Table(table_id) => MatcherReference::Subtable(*table_id),
                    MOIRSymbolId::VarNode(varnode_id) => MatcherReference::Varnode(*varnode_id),
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
            SASTBitPatternConstraint::UnaryOp(_, val) => {
                // TODO implement
                // todo!("Handle unary op (left-right justify/...)")
                let matcher = self.create_matcher(*val)?;
                Ok(matcher)
            }
            SASTBitPatternConstraint::AdvancedConstraint(lhs, op, pattern) => {
                let lhs_symbol = self.spec_builder.get_symbol(&lhs.clone().transmute())?;

                let expression = match lhs_symbol {
                    MOIRSymbolId::Field(field_id) => {
                        self.matcher_from_advanced_constraint(*field_id, op.into_value(), pattern.into_value())
                    }
                    MOIRSymbolId::Table(_) => todo!("Integrate subtable into matcher"),
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
        field_id: FieldId,
        op: SASTConstraintOp,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Box<MatcherExpression>> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or(SleighCompilerError::SymbolUndeclared)?;

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
        field_id: FieldId,
        pattern: SASTConstraintPattern,
    ) -> SleighCompilerResult<Box<MatcherExpression>> {
        let constraint = match pattern {
            SASTConstraintPattern::Ident(ident) => self
                .spec_builder
                .field_id_by_ident(&ident.clone().transmute())
                .map(|field_id| FieldMatcherConstraint::Field(field_id)),
            SASTConstraintPattern::BitPattern(value) => Ok(FieldMatcherConstraint::BitPattern(value.into_value())),
        }?;

        Ok(Box::new(MatcherExpression::Field(FieldMatcher::new(field_id, constraint))))
    }

    /// Generate an or chain of [MatcherExpression]s matching on every possible
    /// value of field except the provided value.
    fn matcher_from_negate_constraint(
        &self,
        field_id: FieldId,
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
        field_id: FieldId,
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
        field_id: FieldId,
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
