use self::matcher::{
    Matcher,
    MatcherAtom,
    OffsettedSubmatchers,
};
use crate::compiler::build::constructor::matcher::MatcherReference;
use crate::compiler::build::trace_build;
use crate::compiler::{
    InternalCompilerResult,
    SleighCompiler,
};
use crate::spec::builder::{
    CustomInterner,
    MOIRSymbolId,
};
use crate::spec::moir::actions::{
    MOIRAction,
    MOIRPatternExpression,
};
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::display::{
    MOIRDisplay,
    MOIRDisplayFragment,
};
use crate::spec::moir::field::{
    FieldId,
    MOIRFieldParent,
};
use crate::spec::moir::field_extractor::FieldExtractor;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::{
    MOIRTable,
    TableId,
};
use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRReference,
    MOIRSlice,
};
use itertools::Itertools;
use matcher::{
    MatcherConjunction,
    MatcherToken,
};
use mogi_sleigh_parser::ast::common::SASTIdent;
use mogi_sleigh_parser::ast::constructor::SASTConstructor;
use mogi_sleigh_parser::ast::constructor::disassembly_section::{
    SASTDisassemblySection,
    SASTPatternExpression,
};
use mogi_sleigh_parser::ast::constructor::display_section::{
    SASTDisplayFragment,
    SASTDisplaySection,
};
use mogi_sleigh_parser::lexer::InputSpan;
use mogi_sleigh_parser::{
    ParserContext,
    Spanned,
    StringId,
};
use mogi_utilities::addresses::EVAddr;
use mogi_utilities::index_map::MapKey;
use std::collections::HashSet;
use tracing::debug;

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
        let mut matchers =
            self.create_matchers(&constructor.bit_pattern_section.constraint, table_id, constructor_id)?;
        trace_build!("Created {} matchers", matchers.len());

        let mut references = HashSet::new();
        matchers.iter().for_each(|matcher| {
            references.extend(matcher.references.clone());
        });
        let references = references.into_iter().collect_vec();

        let display_string = constructor.display_section.display_str.clone().into_value();
        let display_section = constructor.display_section;

        let mut scope = self
            .pc_create_scope(display_section.identifiers(), &references)
            .map_err(|e| e.span("constructor", span))?;

        // if we are in the root table, the first fragment is always interpreted as a mnemonic,
        // i.e. literal
        let is_root_table = table_id == TableId::try_from_index(0).unwrap();
        let display = self.pc_process_display_section(display_section, &mut scope, is_root_table)?;

        let actions = self.pc_process_actions(&mut scope, constructor.disassembly_section)?;
        let modifies_context = actions.iter().any(|action| action.modifies_context());
        let mut moir_constructor = MOIRConstructor {
            local_scope: scope,
            instructions: vec![],
            block_end: false,
            display_string,
            display,
            actions,
            modifies_context,
            export: None,
        };

        trace_build!("{}", moir_constructor.debug_scope());

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        (moir_constructor.export, moir_constructor.block_end) = self.atomize_pcode(
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

        self.pc_populate_sublists_and_extractors(&mut matchers, &moir_constructor.local_scope, table_id);
        self.pc_pad_and_sort_context(&mut matchers);

        // Define the constructor and insert it into the corresponding table.
        self.spec_builder.define_constructor(moir_constructor)?;

        self.register_matchers(constructor.table.transpose(), matchers)?;

        let table = self
            .spec_builder
            .create_or_modify_table(constructor.table.transpose())?;
        table.constructors.push(constructor_id);
        trace_build!("Constructor with id {:?} added to table {:?}", constructor_id, table_name);

        // Catch any errors with exports
        let table = self
            .spec_builder
            .resolve_table(constructor.table.transpose())
            .expect("We just inserted this table");
        self.pc_validate_exports(table)?;

        Ok(())
    }

    fn pc_action_target(
        &self,
        scope: &mut MOIRScope,
        sast_action_lhs: Spanned<SASTIdent>,
    ) -> InternalCompilerResult<(MOIRLocalId, Option<FieldId>)> {
        let local_id = self.pc_resolve_local(scope, sast_action_lhs)?;

        // local can be a context field or a temporary which we need to check for
        let slice = scope.resolve_local(local_id);
        if let MOIRReference::Field(field_id) = slice.reference {
            let field = self.spec_builder.field_by_id(field_id);
            if matches!(field.parent, MOIRFieldParent::Context(_)) {
                // context field
                return Ok((local_id, Some(field_id)));
            }
        }

        // not a context field
        Ok((local_id, None))
    }

    /// Convert a [SASTPatternExpression] to a [MOIRPatternExpression] while resolving referenced
    /// symbols to [MOIRActionLocal]s and collecting all used locals.
    fn pc_convert_expression(
        &self,
        scope: &mut MOIRScope,
        sast_expr: Spanned<SASTPatternExpression>,
        used_locals: &mut Vec<MOIRLocalId>,
    ) -> InternalCompilerResult<MOIRPatternExpression> {
        let (sast_expr, _) = sast_expr.into_tuple();
        Ok(match sast_expr {
            SASTPatternExpression::Add(lhs, rhs) => MOIRPatternExpression::Add(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Sub(lhs, rhs) => MOIRPatternExpression::Sub(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Mul(lhs, rhs) => MOIRPatternExpression::Mul(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Div(lhs, rhs) => MOIRPatternExpression::Div(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Shl(lhs, rhs) => MOIRPatternExpression::Shl(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Shr(lhs, rhs) => MOIRPatternExpression::Shr(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::And(lhs, rhs) => MOIRPatternExpression::And(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Or(lhs, rhs) => MOIRPatternExpression::Or(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Xor(lhs, rhs) => MOIRPatternExpression::Xor(
                Box::new(self.pc_convert_expression(scope, *lhs, used_locals)?),
                Box::new(self.pc_convert_expression(scope, *rhs, used_locals)?),
            ),
            SASTPatternExpression::Not(expr) => {
                MOIRPatternExpression::Not(Box::new(self.pc_convert_expression(scope, *expr, used_locals)?))
            }
            SASTPatternExpression::Symbol(ident) => {
                // TODO correct span
                let local_id = self.pc_resolve_local(scope, Spanned::new(ident, InputSpan::default()))?;
                used_locals.push(local_id);
                MOIRPatternExpression::Local(local_id)
            }
            SASTPatternExpression::Integer(value) => MOIRPatternExpression::Integer(value.try_into().unwrap()),
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
                match self.spec_builder.get_symbol(&ident.transmute()) {
                    Ok(symbol_id) => {
                        // previously defined as symbol => check if it's a context field
                        // and if so, store as local
                        match symbol_id {
                            MOIRSymbolId::Field(field_id) => {
                                let field = self.spec_builder.field_by_id(*field_id);
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

                                        
                                        scope.declare_local(ident.into_value().into(), slice)
                                    }
                                }
                            }
                            _ => {
                                // not a field => error
                                todo!("Generate error report for non-context field symbol in left-hand side of action")
                            }
                        }
                    }
                    Err(_) => {
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
                    let (local_id, context_field) = self.pc_action_target(scope, sast_action.lhs)?;

                    // TODO do this without cloning since we have ownership
                    let mut expression_locals = Vec::new();
                    let expression =
                        self.pc_convert_expression(scope, sast_action.rhs.clone(), &mut expression_locals)?;

                    // mark locals
                    for expression_local in expression_locals.into_iter() {
                        trace_build!("Marking action local {} as used", expression_local);
                        scope.mark_local_as_used(MOIRLocal::Local(expression_local))?;
                    }

                    trace_build!("Marking action local {} as used", local_id);
                    scope.mark_local_as_used(MOIRLocal::Local(local_id))?;

                    Ok(MOIRAction::new(local_id, context_field, expression))
                })
                .collect::<InternalCompilerResult<Vec<_>>>()?;

            Ok(moir_actions)
        } else {
            Ok(vec![])
        }
    }

    fn populate_matcher(
        &self,
        matcher: &mut Matcher,
        scope: &MOIRScope,
        table_id: TableId,
    ) {
        // populate used context fields of matcher from scope
        for field_id in scope.get_used_fields() {
            let field = self.spec_builder.field_by_id(field_id);
            if matches!(field.parent, MOIRFieldParent::Context(_)) {
                matcher.context_fields.push(field_id);
            }
        }

        // even if bytes_consumed is known, partially constrained sublists can still determine
        // the number of bytes consumed which is why if there is any padding at
        // all, we make all sublists dynamic except in the following cases:
        // - recursive entries at the initial position (to avoid infinite recursion)
        // - padding at the end (since nothing follows that's not a problem)
        let mut bytes_consumed = Some(0);
        let contains_padding = matcher
            .conjunctions
            .iter()
            .enumerate()
            .any(|(i, conjunction)| {
                conjunction.token.parent.is_none()
                    && conjunction.atoms.is_empty()
                    && matcher.conjunctions[i..]
                        .iter()
                        .any(|fc| fc.token.parent.is_some() || !fc.atoms.is_empty())
            });

        // process non-context
        for (i, conjunction) in matcher
            .conjunctions
            .iter_mut()
            .skip_while(|conjunction| matches!(conjunction.token.parent, Some(MOIRFieldParent::Context(_))))
            .enumerate()
        {
            for atom in &conjunction.atoms {
                match atom {
                    MatcherAtom::Field { field_id, matcher: _ } => {
                        let field = self.spec_builder.field_by_id(*field_id);

                        match bytes_consumed {
                            Some(bytes_consumed) => {
                                // field used and fixed offset
                                let fe =
                                    FieldExtractor::new(*field_id, bytes_consumed, field.bits, field.signed);
                                matcher.static_field_extractors.push(fe);
                            }
                            None => {
                                // field used and dynamic offset
                                let fe = FieldExtractor::new(*field_id, 0, field.bits, field.signed);
                                conjunction.dynamic_field_extractors.push(fe);
                            }
                        }
                    }
                    MatcherAtom::Submatchers(submatchers) => {
                        if i == 0 && submatchers.table == table_id {
                            // recursion in initial position => make static with offset of zero
                            let os = OffsettedSubmatchers::new(0, submatchers.clone());
                            matcher.static_submatchers.push(os);
                        } else if contains_padding {
                            // padding => assume unknown size
                            conjunction.dynamic_submatchers.push(submatchers.clone());
                        } else {
                            match bytes_consumed {
                                Some(bytes_consumed) => {
                                    // known size
                                    let os = OffsettedSubmatchers::new(bytes_consumed, submatchers.clone());
                                    matcher.static_submatchers.push(os);
                                }
                                None => {
                                    // unknown size
                                    conjunction.dynamic_submatchers.push(submatchers.clone());
                                }
                            }
                        }
                    }
                }
            }

            // update bytes_consumed
            let token_size = conjunction.token.parent.map(|token| {
                let MOIRFieldParent::Token(token) = token else {
                    panic!("We checked for this above")
                };

                self.spec_builder.token_by_id(token).bytes
            });

            match token_size {
                Some(token_size) => {
                    if let Some(bytes_consumed) = &mut bytes_consumed {
                        *bytes_consumed += usize::from(token_size);
                    }
                }
                None => bytes_consumed = None,
            }
        }

        // remove empty
        matcher
            .conjunctions
            .retain(|conjunction| conjunction.token.parent.is_some() || !conjunction.atoms.is_empty());
    }

    /// Populate matcher conjunctions with dynamic-offset sublists and field extractors
    /// and populate the matcher itself with fixed-offset sublists and field extractors.
    /// Also remove empty conjunctions that arose from padding.
    fn pc_populate_sublists_and_extractors(
        &self,
        matchers: &mut Vec<Matcher>,
        scope: &MOIRScope,
        // for detection of recursion
        table_id: TableId,
    ) {
        for matcher in matchers.iter_mut() {
            self.populate_matcher(matcher, scope, table_id);
        }
    }

    /// Jumplist creation is made easier if all matchers begin with a uniform context since special
    /// treatments for context and non-context matchers might create priority issues if they
    /// overlap, i.e. one context constrains for zero.
    fn pc_pad_and_sort_context(
        &self,
        matchers: &mut Vec<Matcher>,
    ) {
        for matcher in matchers.iter_mut() {
            let mut padded_conjunctions = self
                .spec_builder
                .context_ids()
                .map(|cid| {
                    MatcherConjunction::new(
                        Default::default(),
                        MatcherToken {
                            parent: Some(MOIRFieldParent::Context(cid)),
                        },
                    )
                })
                .collect::<Vec<_>>();

            let matcher_conjunctions = std::mem::take(&mut matcher.conjunctions);

            for matcher_conjunction in matcher_conjunctions.into_iter() {
                if let Some(MOIRFieldParent::Context(cid)) = matcher_conjunction.token.parent {
                    // is context => add atoms to padded_conjunctions
                    padded_conjunctions
                        .get_mut(cid.to_index())
                        .expect("This must exist")
                        .atoms = matcher_conjunction.atoms;
                } else {
                    // is token => add conjunction
                    padded_conjunctions.push(matcher_conjunction);
                }
            }

            matcher.conjunctions = padded_conjunctions;
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
                let existing_constructor = self.spec_builder.constructor_by_id(*existing_constructor);
                let new_constructor = table
                    .constructors
                    .last()
                    .expect("If there is a first element, there has to be a last element");
                let new_constructor = self.spec_builder.constructor_by_id(*new_constructor);
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

    /// Resolve a [SASTDisplayFragment] to a [MOIRDisplayFragment].
    fn resolve_fragment(
        &self,
        sast_fragment: Spanned<SASTDisplayFragment>,
        scope: &MOIRScope,
        to_literal: bool,
    ) -> InternalCompilerResult<Option<MOIRDisplayFragment>> {
        let fragment = sast_fragment.into_value();

        Ok(match fragment {
            SASTDisplayFragment::Literal(string) => Some(MOIRDisplayFragment::Literal(string)),
            SASTDisplayFragment::Identifier(sid) => {
                if to_literal {
                    let string = self
                        .spec_builder
                        .interner()
                        .string(sid)
                        .expect("If we stored it it must be interned");
                    Some(MOIRDisplayFragment::Literal(string.to_string()))
                } else {
                    let local_id = scope
                        .local_id_by_ident(sid)
                        .expect("Fragment identifiers must be included in the scope");
                    let local = scope
                        .local(sid)
                        .expect("Fragmet identifiers must be included in the scope");

                    match local.reference {
                        MOIRReference::Field(_)
                        | MOIRReference::SubTable(_)
                        | MOIRReference::VarNode(_)
                        | MOIRReference::Const(_) => Some(MOIRDisplayFragment::Local(local_id)),
                        _ => todo!("Generate error report for invalid reference type in display string"),
                    }
                }
            }
            SASTDisplayFragment::Whitespace => Some(MOIRDisplayFragment::Literal(" ".to_string())),
            SASTDisplayFragment::Hat => None,
        })
    }

    /// A [SASTDisplaySection] is processed into a [MOIRDisplay] by resolving identifiers.
    fn pc_process_display_section(
        &mut self,
        display_section: Spanned<SASTDisplaySection>,
        scope: &mut MOIRScope,
        is_root_table: bool,
    ) -> InternalCompilerResult<MOIRDisplay> {
        let len = display_section.value().fragments.len();
        let moir_fragments: Vec<MOIRDisplayFragment> = display_section
            .into_value()
            .fragments
            .into_iter()
            .enumerate()
            .filter_map(|(i, sast_fragment)| {
                if (i == 0 || i + 1 == len) && matches!(sast_fragment.value(), SASTDisplayFragment::Whitespace) {
                    // skip start and end
                    return None;
                }

                self.resolve_fragment(sast_fragment, scope, is_root_table && i == 0)
                    .transpose()
            })
            .collect::<InternalCompilerResult<_>>()?;

        // mark fragment locals as used
        for fragment in moir_fragments.iter() {
            if let MOIRDisplayFragment::Local(local_id) = fragment {
                scope.mark_local_as_used(MOIRLocal::Local(*local_id))?;
            }
        }

        Ok(MOIRDisplay::new(moir_fragments))
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
                scope.allocate_named_const(ident, MOIRMaybeSize::Undetermined)?;
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
        trace_build!("Resolving {} symbols to a local scope", references.len());
        for reference in references {
            trace_build!("Resolving symbol {reference:?}");
            let value = match reference {
                MatcherReference::Field(field_id) => {
                    let field = self.spec_builder.field_by_id(*field_id);
                    let size = if !field.variables.is_empty() {
                        // The field has variables attached. These will be later given to the constructor during
                        // decoding. We can now set the size of the value that is referenced to the size of the
                        // attached varnode.
                        let vnid = *field
                            .variables
                            .first()
                            .expect("We just made sure there is at least one element");
                        self.spec_builder.varnode_by_id(vnid).size.into()
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
                    let table = self.spec_builder.table_by_id(*stid);
                    let constructor_id = table.constructors.first();
                    let export_size = constructor_id
                        .map(|cid| self.spec_builder.constructor_by_id(*cid).export_size())
                        // If the constructor was not found, we set the export to zero size.
                        .unwrap_or(MOIRMaybeSize::Determined(0));
                    Some(MOIRSlice {
                        reference: MOIRReference::SubTable(*stid),
                        start: 0,
                        end: export_size,
                    })
                }
                MatcherReference::Varnode(vnid) => {
                    let varnode = self.spec_builder.varnode_by_id(*vnid);
                    Some(MOIRSlice {
                        reference: MOIRReference::VarNode(*vnid),
                        start: 0,
                        end: MOIRMaybeSize::Determined(varnode.size),
                    })
                }
            };

            let ident = self.spec_builder.get_string(MOIRSymbolId::from(reference));
            if let Some(value) = value {
                scope.declare_local(ident.into_value(), value);
            }
        }
        Ok(())
    }
}
