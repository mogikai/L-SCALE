use std::collections::HashSet;

use crate::compiler::ast::constructor::SASTConstructor;
use crate::compiler::ast::Spanned;
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
use crate::spec::moir::code::{
    MOIRInstruction,
    MOIROperation,
};
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRMaybeSize,
    MOIRScope,
};
use crate::spec::moir::table::MOIRExport;
use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRReference,
    MOIRSlice,
};
use crate::spec::ProcessorSpecificationError;
use tracing::debug;

mod matcher_builder;

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
        trace_build!("Defining constructor in table {:?}", table_name);

        // TODO: Need to parse the disassembly section here as well and add that information to the matcher
        let matcher = self.create_matcher(constructor.bit_pattern_section.into_value().constraint)?;

        let display_section = constructor.display_section.into_value();
        let scope =
            self.pc_create_scope(display_section.identifiers, &matcher.symbols_used)?;

        let mut moir_constructor = MOIRConstructor {
            local_scope: scope,
            instructions: vec![],
            display: display_section.display_str.into_value(),
        };
        trace_build!("{}", moir_constructor.debug_scope());

        // The semantic section contains pcode which has to be converted into an atomic form.
        // Pcode can contain complex statements (e.g. x = (a+b) * c)) which have to be translated
        // into a form that consists only of atomic expressions (e.g. tmp = a + b; x = tmp * c;)
        let export = self.atomize_pcode(
            &mut moir_constructor.local_scope,
            &mut moir_constructor.instructions,
            constructor.semantic_section.into_value().0,
        )?;
        self.pc_mark_used_locals(&mut moir_constructor.local_scope, &moir_constructor.instructions)?;
        trace_build!("{}", moir_constructor.jcode_debug_string());
        self.pc_handle_export(constructor.table.transpose(), export)?;
        let _id = self.spec_builder.define_constructor(moir_constructor)?;
        // TODO: Insert the constructor id into the corresponding matcher and/or the table.
        Ok(())
    }

    fn pc_mark_used_locals(
        &mut self,
        scope: &mut MOIRScope,
        instructions: &[MOIRInstruction],
    ) -> SleighCompilerResult {
        for instruction in instructions {
            if let Some(output) = instruction.output {
                scope.mark_local_as_used(MOIRLocal::from(output))?;
            }
            if let Some(input_a) = instruction.input_a {
                scope.mark_local_as_used(input_a)?;
            }
            if let Some(input_b) = instruction.input_b {
                scope.mark_local_as_used(input_b)?;
            }
            if let MOIROperation::CustomOp { extra_args, .. } = &instruction.operation {
                for arg in extra_args {
                    scope.mark_local_as_used(*arg)?;
                }
            }
        }
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

        matchers.into_iter().for_each(|matcher| {
            // TODO matcher.constructor =
            table.matchers.insert(matcher);
        });

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
    ) -> SleighCompilerResult {
        trace_build!("Resolving {} symbols to a local scope", symbols.len());
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
                            end: MOIRMaybeSize::Determined(varnode.size as u8),
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
}
