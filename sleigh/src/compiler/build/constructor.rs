use super::matcher::{
    FieldMatcher,
    FieldMatcherConstraint,
    Matcher,
    MatcherExpression,
    MatcherReference,
};
use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::constructor::bit_pattern_section::{
    SASTBinaryOp,
    SASTBitPatternConstraint,
    SASTConstraintOp,
    SASTConstraintPattern,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::{
    InternalError,
    InternalErrorType,
};
use crate::compiler::{
    CompilerContext,
    SleighCompiler,
    InternalCompilerResult,
};
use crate::spec::ProcessorSpecificationError;
use crate::spec::builder::{
    MOIRSymbolId,
    StringId,
};
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::{
    MOIRExport,
    MOIRTable,
};
use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRReference,
    MOIRSlice,
};
use tracing::debug;

use crate::spec::ProcessorSpecificationError::SymbolUndeclared;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if it does not exist.
    /// Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        constructor: Spanned<SASTConstructor>,
    ) -> InternalCompilerResult {
        let (constructor, constructor_span) = constructor.into_tuple();
        #[allow(unused_variables)]
        let table_name = constructor
            .table
            .value()
            .map(|sid| self.spec_builder.string(sid))
            .flatten()
            .unwrap_or("instruction")
            .to_string();
        trace_build!("=======================================");
        trace_build!("Defining constructor in table {:?}", table_name);

        // TODO: Need to parse the disassembly section here as well and add that information to the matcher
        let matcher = self.create_matcher(constructor.bit_pattern_section.into_value().constraint)?;

        let display_section = constructor.display_section.into_value();
        let scope = self
            .pc_create_scope(display_section.identifiers, &matcher.symbols_used)
            .map_err(|e| e.span("constructor", constructor_span))?;

        let mut moir_constructor = MOIRConstructor {
            local_scope: scope,
            instructions: vec![],
            display: display_section.display_str.into_value(),
            export: None,
        };
        trace_build!("{}", moir_constructor.debug_scope());

        // TODO: we need to walk the constraint and find out the tokens that are used.
        //       we also need to make sure that there is only one token per section
        //       delimited by a semicolon.

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        moir_constructor.export = self.atomize_pcode(
            &mut moir_constructor.local_scope,
            &mut moir_constructor.instructions,
            constructor.semantic_section.into_value().0,
        )?;
        debug!("Constructor is exporting {:?}", moir_constructor.export);
        moir_constructor.mark_used_locals()?;
        // TODO: This needs to be moved to the moir->jir stage
        // moir_constructor
        //     .local_scope
        //     .realize_temporaries(&mut self.spec_builder)?;
        trace_build!("{}", moir_constructor.mcode_debug_string(self.spec_builder.interner()));

        // Define the constructor and insert it into the corresponding table.
        let id = self.spec_builder.define_constructor(moir_constructor)?;
        let table_id = constructor.table.transpose();
        let table = self.spec_builder.create_or_modify_table(table_id)?;
        table.constructors.push(id);
        trace_build!("Construtor with id {:?} added to table {:?}", id, table_name);

        // Catch any errors with exports
        let table = self
            .spec_builder
            .table_by_ident(table_id)
            .expect("We just inserted this table");
        self.pc_validate_exports(table)?;
        Ok(())
    }

    /// Validates the constructors in the table.
    /// If the newly added constructor does not agree with the previous constructors, an error is reported.
    fn pc_validate_exports(
        &self,
        table: &MOIRTable,
    ) -> InternalCompilerResult {
        // TODO: If we save some more span information in the MOIR structs, then we can move this validation
        //       step to the final build phase. This would allow us to report all conflicting constructors.
        //       It would also make the runtime more efficient, since we only have to validate once.
        match table.constructors.first() {
            None => {
                // If there are no previous constructor definitions, we can simply return
                Ok(())
            }
            Some(existing_constructor) => {
                let existing_constructor = self
                    .spec_builder
                    .constructor_by_symbol_id(*existing_constructor)
                    .expect("The constructor should exist since we have an id for it");
                let new_constructor = table
                    .constructors
                    .last()
                    .expect("If there is a first element, there has to be a last element");
                let new_constructor = self
                    .spec_builder
                    .constructor_by_symbol_id(*new_constructor)
                    .expect("The constructor should exist since we have an id for it");
                match existing_constructor.export.as_ref() {
                    None => {
                        if new_constructor.export.is_some() {
                            todo!(
                                "Generate error: 'Defining a new constructor with an export statement is disallowed when the previous definitions do not export'"
                            )
                        }
                        Ok(())
                    }
                    Some(existing_export) => {
                        // If the table already contains a constructor that exports, we have to make sure that the sizes of all
                        // exports match. Also, it is not allowed, that a constructor has no export statement, if
                        // another constructor has an export statement. If one constructor exports, all of them have to.
                        let Some(export) = new_constructor.export.as_ref() else {
                            todo!(
                                "Generate error: 'Defining a new constructor without an export statement is disallowed when the previous definitions already export'"
                            )
                        };
                        let export_size = export.size(&new_constructor.local_scope);
                        let existing_export_size = existing_export.size(&existing_constructor.local_scope);
                        if export_size != existing_export_size {
                            debug!(
                                "Existing export size {existing_export_size} does not match new export size {export_size}"
                            );
                            todo!("Generate error report for inconsistent sizes of exports")
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    fn pc_create_scope(
        &mut self,
        display_identifiers: Vec<Spanned<StringId>>,
        matcher_symbols: &[MOIRSymbolId],
    ) -> Result<MOIRScope, InternalError> {
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
                #[allow(unused_variables)]
                let str = self.spec_builder.resolve_string_id(*ident.value());
                trace_build!("Local scope is missing identifier {str}. Allocating it now.");
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
    ) -> InternalCompilerResult {
        trace_build!("Resolving {} symbols to a local scope", symbols.len());
        for symbol in symbols {
            trace_build!("Resolving symbol {symbol:?}");
            let value = match symbol {
                MOIRSymbolId::Field(field_id) => {
                    let field = self
                        .spec_builder
                        .field_by_symbol_id(*field_id)
                        .ok_or(InternalErrorType::UndeclaredSymbol)?;
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
                        .ok_or(InternalErrorType::UndeclaredSymbol)?;
                    let constructor_id = table.constructors.first();
                    let export_size = constructor_id
                        .map(|cid| self.spec_builder.constructor_by_symbol_id(*cid))
                        .flatten()
                        .map(|c| c.export_size())
                        // If the constructor was not found, we set the export to zero size.
                        .unwrap_or(MOIRMaybeSize::Determined(0));
                    Some(MOIRSlice {
                        reference: MOIRReference::SubTable(*table_id),
                        start: 0,
                        end: export_size,
                    })
                }

                MOIRSymbolId::VarNode(vnid) => {
                    let varnode = self
                        .spec_builder
                        .varnode_by_symbol_id(*vnid)
                        .ok_or(InternalErrorType::UndeclaredSymbol)?;
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
                .ok_or(InternalErrorType::UndeclaredSymbol)?;
            if let Some(value) = value {
                scope.declare_local(ident.clone().into_value(), value);
            }
        }

        Ok(())
    }

    fn create_matcher(
        &mut self,
        constraint: Spanned<SASTBitPatternConstraint>,
    ) -> InternalCompilerResult<Matcher> {
        let (constraint, constraint_span) = constraint.into_tuple();
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
                    MOIRSymbolId::Field(field_id) => self
                        .matcher_from_advanced_constraint(*field_id, op.into_value(), pattern.into_value())
                        .map_err(|e| e.span("constraint", constraint_span)),
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
    ) -> InternalCompilerResult<Box<MatcherExpression>> {
        let field = self
            .spec_builder
            .field_by_symbol_id(field_id)
            .ok_or(InternalErrorType::UndeclaredSymbol)?;

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
    ) -> InternalCompilerResult<Box<MatcherExpression>> {
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
