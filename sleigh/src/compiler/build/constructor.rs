use self::matcher::Matcher;
use crate::compiler::ast::Spanned;
use crate::compiler::ast::common::SASTIdent;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::constructor::disassembly_section::{
    SASTDisassemblySection,
    SASTPatternExpression,
};
use crate::compiler::build::constructor::matcher::MatcherReference;
use crate::compiler::build::trace_build;
use crate::compiler::error::{
    InternalError,
    InternalErrorType,
    SleighCompilerError,
};
use crate::compiler::lexer::InputSpan;
use crate::compiler::{
    CompilerContext,
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::builder::{
    MOIRSymbolId,
    StringId,
};
use crate::spec::moir::actions::{
    MOIRAction,
    MOIRActionTarget,
    MOIRPatternExpression,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::field::MOIRFieldParent;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRTable;
use crate::spec::moir::varnode::{
    MOIRReference,
    MOIRSlice,
};
use itertools::Itertools;
use mogi_utilities::addresses::EVAddr;
use std::collections::HashSet;
use tracing::{
    debug,
    trace,
    warn,
};

pub(crate) mod matcher;
pub(crate) mod matcher_builder;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if
    /// it does not exist. Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        spanned_constructor: Spanned<SASTConstructor>,
    ) -> InternalCompilerResult {
        let (constructor, span) = spanned_constructor.into_tuple();
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

        let table_id = self
            .spec_builder
            .get_table_id_or_create(constructor.table.transpose())?;

        let constructor_id = self.spec_builder.next_constructor_id();
        let matchers = self.create_matchers(&constructor.bit_pattern_section.constraint, table_id, constructor_id)?;

        let mut references = HashSet::new();
        matchers.iter().for_each(|matcher| {
            references.extend(matcher.references.clone());
        });
        let references = references.into_iter().collect_vec();

        let display_section = constructor.display_section.into_value();
        let mut scope = self
            .pc_create_scope(display_section.identifiers, &references)
            .map_err(|e| e.span("constructor", span))?;
        let actions = self.pc_process_actions(&mut scope, constructor.disassembly_section)?;
        let mut moir_constructor = MOIRConstructor {
            local_scope: scope,
            instructions: vec![],
            display: display_section.display_str.into_value(),
            export: None,
            actions,
        };
        trace_build!("{}", moir_constructor.debug_scope());

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        moir_constructor.export = self.atomize_pcode(
            &mut moir_constructor.local_scope,
            &mut moir_constructor.instructions,
            constructor.semantic_section.into_value().0,
        )?;

        trace_build!("Constructor is exporting {:?}", moir_constructor.export);
        moir_constructor.mark_used_locals()?;
        // TODO: This needs to be moved to the moir->jir stage
        // moir_constructor
        //     .local_scope
        //     .realize_temporaries(&mut self.spec_builder)?;
        trace_build!("{}", moir_constructor.mcode_debug_string(self.spec_builder.interner()));

        // Define the constructor and insert it into the corresponding table.
        self.spec_builder.define_constructor(moir_constructor)?;

        self.register_matchers(constructor.table.clone().transpose(), matchers)?;

        let table = self
            .spec_builder
            .create_or_modify_table(constructor.table.transpose())?;
        table.constructors.push(constructor_id);
        trace_build!("Constructor with id {:?} added to table {:?}", constructor_id, table_name);

        // Catch any errors with exports
        let table = self
            .spec_builder
            .table_by_ident(constructor.table.transpose())
            .expect("We just inserted this table");
        self.pc_validate_exports(table)?;

        Ok(())
    }

    fn pc_action_target(
        &self,
        scope: &mut MOIRScope,
        sast_action_lhs: Spanned<SASTIdent>,
    ) -> InternalCompilerResult<MOIRActionTarget> {
        let local_id = self.pc_resolve_local(scope, sast_action_lhs)?;

        // local can be a context field or a temporary which we need to check for
        let slice = scope.resolve_local(local_id)?;
        if let MOIRReference::Field(field_id) = slice.reference {
            let field = self
                .spec_builder
                .field_by_symbol_id(field_id)
                .ok_or_else(|| -> InternalError { todo!("Generate error report for missing field") })?;
            if matches!(field.parent, MOIRFieldParent::Context(_)) {
                // context field
                return Ok(MOIRActionTarget::ContextField(field_id));
            }
        }

        // not a context field
        Ok(MOIRActionTarget::Local(local_id))
    }

    /// Convert a [SASTPatternExpression] to a [MOIRPatternExpression] while resolving referenced
    /// symbols to [MOIRActionLocal]s.
    fn pc_convert_expression(
        &self,
        scope: &mut MOIRScope,
        sast_expr: Spanned<SASTPatternExpression>,
    ) -> InternalCompilerResult<MOIRPatternExpression> {
        let (sast_expr, span) = sast_expr.into_tuple();
        Ok(match sast_expr {
            SASTPatternExpression::Add(lhs, rhs) => MOIRPatternExpression::Add(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Sub(lhs, rhs) => MOIRPatternExpression::Sub(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Mul(lhs, rhs) => MOIRPatternExpression::Mul(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Div(lhs, rhs) => MOIRPatternExpression::Div(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Shl(lhs, rhs) => MOIRPatternExpression::Shl(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Shr(lhs, rhs) => MOIRPatternExpression::Shr(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::And(lhs, rhs) => MOIRPatternExpression::And(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Or(lhs, rhs) => MOIRPatternExpression::Or(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Xor(lhs, rhs) => MOIRPatternExpression::Xor(
                Box::new(self.pc_convert_expression(scope, *lhs)?),
                Box::new(self.pc_convert_expression(scope, *rhs)?),
            ),
            SASTPatternExpression::Not(expr) => {
                MOIRPatternExpression::Not(Box::new(self.pc_convert_expression(scope, *expr)?))
            }
            SASTPatternExpression::Symbol(ident) => {
                // TODO correct span
                MOIRPatternExpression::Local(self.pc_resolve_local(scope, Spanned::new(ident, InputSpan::default()))?)
            }
            SASTPatternExpression::Integer(value) => MOIRPatternExpression::Integer(value as u64),
        })
    }

    /// When parsing the disassembly actions, not all identifiers must have
    /// been previously stored as locals. There are two cases:
    /// - ident appears in lhs or rhs as a symbol, more specifically a context field
    /// - ident appears in lhs and is not a symbol
    /// It is an error for ident to appear in rhs if it has not been previously
    /// defined. It is also an error for previously defined symbols that are not
    /// context fields to appear in lhs or rhs.
    ///
    /// This function will modify the scope appropriately and return the resolved local
    /// id.
    fn pc_resolve_local(
        &self,
        scope: &mut MOIRScope,
        ident: Spanned<SASTIdent>,
    ) -> InternalCompilerResult<MOIRLocalId> {
        Ok(match scope.local_id_by_ident(ident.into_value().into()) {
            Some(local_id) => {
                // already defined => everything good, use as-is
                local_id
            }
            None => {
                // ident did not previously appear as local => check for the two cases defined above
                match self.spec_builder.get_symbol_and_ident(&ident.transmute()) {
                    Some((symbol_id, _)) => {
                        // previously defined as symbol => check if it's a context field
                        // and if so, store as local
                        match symbol_id {
                            MOIRSymbolId::Field(field_id) => {
                                let field = self.spec_builder.field_by_symbol_id(*field_id).ok_or_else(
                                    || -> InternalError { todo!("Generate error report for missing field") },
                                )?;
                                match field.parent {
                                    MOIRFieldParent::Token(_) => {
                                        // not a context field => error
                                        todo!("Generate error report for token field in left-hand side of action");
                                    }
                                    MOIRFieldParent::Context(_) => {
                                        let reference = MOIRReference::Field(*field_id);
                                        let slice = MOIRSlice {
                                            reference,
                                            start: 0,
                                            end: MOIRMaybeSize::Undetermined,
                                        };

                                        let id = scope.declare_local(ident.into_value().into(), slice);
                                        id
                                    }
                                }
                            }
                            _ => {
                                // not a field => error
                                todo!("Generate error report for non-context field symbol in left-hand side of action")
                            }
                        }
                    }
                    None => {
                        // not previously defined as symbol => allocate new const
                        scope.allocate_named_const(ident.transmute(), MOIRMaybeSize::Undetermined)?
                    }
                }
            }
        })
    }

    /// Process the disassembly actions section into a list of [MOIRAction].
    fn pc_process_actions(
        &self,
        scope: &mut MOIRScope,
        actions_section: Spanned<SASTDisassemblySection>,
    ) -> InternalCompilerResult<Vec<MOIRAction>> {
        if let Some(sast_actions) = actions_section.into_value().actions {
            let moir_actions = sast_actions
                .into_iter()
                .map(|sast_action| {
                    let target = self.pc_action_target(scope, sast_action.lhs)?;

                    // TODO do this without cloning since we have ownership
                    let expression = self.pc_convert_expression(scope, sast_action.rhs.clone())?;
                    Ok(MOIRAction::new(target, expression))
                })
                .collect::<InternalCompilerResult<Vec<_>>>()?;

            Ok(moir_actions)
        } else {
            Ok(vec![])
        }
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

    fn register_matchers(
        &mut self,
        table: Option<Spanned<StringId>>,
        matchers: Vec<Matcher>,
    ) -> InternalCompilerResult {
        let table = self.spec_builder.create_or_modify_table(table)?;

        // only add new matchers if there are no duplicates
        matchers.into_iter().for_each(|matcher| {
            if !table.matchers.values().contains(&matcher) {
                table.matchers.insert(matcher);
            }
        });

        Ok(())
    }

    fn pc_create_scope(
        &mut self,
        display_identifiers: Vec<Spanned<StringId>>,
        references: &[MatcherReference],
    ) -> InternalCompilerResult<MOIRScope> {
        // Declare the special identifiers that are filled with constants upon calling the constructor.
        // Their sizes are undetermined and inferred from context.
        let inst_start = self.spec_builder.intern("inst_start");
        let inst_next = self.spec_builder.intern("inst_next");
        let inst_next2 = self.spec_builder.intern("inst_next2");

        // TODO where to get this from at this point?
        let addr_bytes = EVAddr::<u8>::bits() / 8;
        let mut scope = MOIRScope::new(inst_start, inst_next, inst_next2, addr_bytes.try_into().unwrap());

        self.resolve_references_to_local_scope(&mut scope, references)?;
        trace_build!("display_identifiers={:?}", display_identifiers);
        for ident in display_identifiers {
            // TODO: We probably need to figure out a better way to handle these locals that are constructed
            //       in the disassembly section.
            let local = scope.local(*ident.value());
            if local.is_none() {
                #[allow(unused_variables)]
                let str = self.spec_builder.resolve_string_id(*ident.value());
                trace_build!("Local scope is missing identifier {str}. Allocating it now.");

                // display identifiers don't have a known size
                scope.allocate_named_const(ident.clone(), MOIRMaybeSize::Undetermined)?;
            }
        }
        Ok(scope)
    }

    /// Resolve a given list of [MatcherReference] used by [Matcher] in its bit pattern section to the
    /// [MOIRScope] of the constructor for usage in the semantic section.
    fn resolve_references_to_local_scope(
        &self,
        scope: &mut MOIRScope,
        references: &[MatcherReference],
    ) -> InternalCompilerResult {
        trace!("Resolving {} symbols to a local scope", references.len());
        for reference in references {
            trace_build!("Resolving symbol {reference:?}");
            let value = match reference {
                MatcherReference::Field(field_id) => {
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
                MatcherReference::Subtable(stid) => {
                    let table = self
                        .spec_builder
                        .table_by_symbol_id(*stid)
                        .ok_or(InternalErrorType::UndeclaredSymbol)?;
                    let constructor_id = table.constructors.first();
                    let export_size = constructor_id
                        .map(|cid| self.spec_builder.constructor_by_symbol_id(*cid))
                        .flatten()
                        .map(|c| c.export_size())
                        // If the constructor was not found, we set the export to zero size.
                        .unwrap_or(MOIRMaybeSize::Determined(0));
                    Some(MOIRSlice {
                        reference: MOIRReference::SubTable(*stid),
                        start: 0,
                        end: export_size,
                    })
                }
                MatcherReference::Varnode(vnid) => {
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
            };

            let ident = self
                .spec_builder
                .ident_by_symbol(&MOIRSymbolId::from(reference))
                .ok_or(InternalErrorType::UndeclaredSymbol)?;
            if let Some(value) = value {
                scope.declare_local(ident.clone().into_value(), value);
            }
        }
        Ok(())
    }
}
