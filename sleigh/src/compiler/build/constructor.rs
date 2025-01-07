use self::matcher::Matcher;
use crate::compiler::ast::constructor::disassembly_section::SASTDisassemblySection;
use crate::compiler::ast::Spanned;
use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::build::constructor::matcher::MatcherReference;
use crate::compiler::build::trace_build;
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
use crate::spec::moir::actions::MOIRAction;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRTable;
use crate::spec::moir::varnode::{
    MOIRReference,
    MOIRSlice,
};
use itertools::Itertools;
use std::collections::HashSet;
use tracing::{
    debug,
    trace,
};

pub(crate) mod matcher;
pub(crate) mod matcher_builder;

impl SleighCompiler {
    /// Process a constructor. This will create the table in which the constructor is anchored, if
    /// it does not exist. Then, the bit pattern section is validated.
    pub(super) fn process_constructor(
        &mut self,
        spanned_constructor: Spanned<SASTConstructor>,
    ) -> SleighCompilerResult {
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

        let mut matchers = self.create_matchers(&constructor.bit_pattern_section.constraint, table_id)?;

        let mut references = HashSet::new();
        matchers.iter().for_each(|matcher| {
            references.extend(matcher.references.clone());
        });
        let references = references.into_iter().collect_vec();

        let display_section = constructor.display_section.into_value();
        let scope = self.pc_create_scope(display_section.identifiers, &references)?;
        let actions = self.pc_process_actions(constructor.disassembly_section)?;
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

        debug!("Constructor is exporting {:?}", moir_constructor.export);
        moir_constructor.mark_used_locals()?;
        // TODO: This needs to be moved to the moir->jir stage
        // moir_constructor
        //     .local_scope
        //     .realize_temporaries(&mut self.spec_builder)?;
        trace_build!("{}", moir_constructor.mcode_debug_string(self.spec_builder.interner()));

        let display = moir_constructor.display.clone();

        // Define the constructor and insert it into the corresponding table.
        let id = self.spec_builder.define_constructor(moir_constructor)?;

        // insert the constructor id into the corresponding matchers
        matchers.iter_mut().for_each(|matcher| {
            matcher.constructor = Some(id);
            matcher.display = Some(display.clone()); // TODO remove
        });

        self.register_matchers(constructor.table.clone().transpose(), matchers)?;

        let table = self
            .spec_builder
            .create_or_modify_table(constructor.table.transpose())?;
        table.constructors.push(id);
        trace_build!("Constructor with id {:?} added to table {:?}", id, table_name);

        // Catch any errors with exports
        let table = self
            .spec_builder
            .table_by_ident(constructor.table.transpose())
            .expect("We just inserted this table");
        self.pc_validate_exports(table)?;

        Ok(())
    }

    /// Process the disassembly actions section into a list of [MOIRAction].
    fn pc_process_actions(
        &self,
        actions_section: Spanned<SASTDisassemblySection>
    ) -> SleighCompilerResult<Vec<MOIRAction>> {
        if let Some(sast_actions) = actions_section.into_value().actions {
            // let moir_actions = sast_actions.into_iter().map(|sast_action| {
            //     sast_action
            // })
        }

        Ok(vec![])
    }

    /// Validates the constructors in the table.
    /// If the newly added constructor does not agree with the previous constructors, an error is reported.
    fn pc_validate_exports(
        &self,
        table: &MOIRTable,
    ) -> SleighCompilerResult {
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
    ) -> SleighCompilerResult {
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
    ) -> Result<MOIRScope, SleighCompilerError> {
        let mut scope = MOIRScope::default();
        // Declare the special identifiers that are filled with constants upon calling the constructor.
        // Their sizes are undetermined and inferred from context.
        scope.declare_local(self.spec_builder.intern("inst_start"), MOIRSlice::inst_start());
        scope.declare_local(self.spec_builder.intern("inst_next"), MOIRSlice::inst_next());
        scope.declare_local(self.spec_builder.intern("inst_next2"), MOIRSlice::inst_next2());

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
                scope.allocate_named_temporary(ident.clone(), MOIRMaybeSize::Undetermined)?;
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
    ) -> SleighCompilerResult {
        trace!("Resolving {} symbols to a local scope", references.len());
        for reference in references {
            trace_build!("Resolving symbol {reference:?}");
            let value = match reference {
                MatcherReference::Field(field_id) => {
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
                MatcherReference::Subtable(stid) => {
                    let table = self
                        .spec_builder
                        .table_by_symbol_id(*stid)
                        .ok_or(SleighCompilerError::SymbolUndeclared)?;
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
                        .ok_or(SleighCompilerError::SymbolUndeclared)?;
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
                .ok_or(SleighCompilerError::SymbolUndeclared)?;
            if let Some(value) = value {
                scope.declare_local(ident.clone().into_value(), value);
            }
        }
        Ok(())
    }
}
