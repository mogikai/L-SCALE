use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::InputSpan;
use crate::compiler::SleighCompilerResult;
use crate::new_typed_index_table_id;
use crate::spec::builder::{
    ProcessorSpecificationBuilder,
    StringId,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::varnode::{
    MOIRSlice,
    MOIRSliceEnd,
    MOIRValue,
};
use crate::spec::reporting::generate_identifier_exists_report;
use crate::spec::typed_index_table::TypedIndexTable;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// A scope for locals used in constructors and macros
#[derive(Debug, Default)]
pub struct MOIRScope {
    /// Keeps track of the sizes of temporaries assigned as part of the semantic section.
    temporaries: TypedIndexTable<MOIRTemporaryId, MOIRTemporary>,
    locals: HashMap<StringId, MOIRValue>,
    local_spans: HashMap<StringId, InputSpan>,
}

/// A temporary varnode. These have no backing varnode at this stage yet.
/// They will be mapped to concrete varnodes once the constructor is called.
#[derive(Debug)]
struct MOIRTemporary {
    pub size: MOIRTemporarySize,
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRTemporarySize {
    Determined(MOIRSize),
    Undetermined,
}

impl From<MOIRSize> for MOIRTemporarySize {
    fn from(size: MOIRSize) -> Self {
        MOIRTemporarySize::Determined(size)
    }
}

impl From<MOIRTemporarySize> for MOIRSliceEnd {
    fn from(size: MOIRTemporarySize) -> Self {
        match size {
            MOIRTemporarySize::Determined(size) => MOIRSliceEnd::Determined(size),
            MOIRTemporarySize::Undetermined => MOIRSliceEnd::Undetermined,
        }
    }
}

new_typed_index_table_id! { pub struct MOIRTemporaryId; }

impl MOIRScope {
    pub fn declare_value(
        &mut self,
        ident: StringId,
        value: MOIRValue,
    ) {
        self.locals.insert(ident, value);
    }

    /// Temporaries can be named or unnamed. Named temporaries are created for example by using
    /// the `local` keyword in pcode statements. Unnamed temporaries are created by the compiler
    /// for storing intermediate values when atomizing expressions.
    pub fn allocate_temporary(&mut self) -> MOIRSlice {
        MOIRSlice::temporary(
            self.temporaries.insert(MOIRTemporary {
                size: MOIRTemporarySize::Undetermined,
            }),
            0,
            MOIRSliceEnd::Undetermined,
        )
    }

    /// Try to allocate a named temporary. If the temporary already exists,
    /// an error report is generated.
    pub fn allocate_named_temporary(
        &mut self,
        ident: Spanned<StringId>,
        size: MOIRTemporarySize,
    ) -> SleighCompilerResult {
        let (ident, ident_span) = ident.into_tuple();
        let id = self.temporaries.insert(MOIRTemporary { size });
        match self.locals.entry(ident) {
            Entry::Occupied(_) => Err(generate_identifier_exists_report(
                "",
                self.local_spans
                    .get(&ident)
                    .expect("This must exist, since the locals entry is occupied")
                    .clone(),
                ident_span,
            )),
            Entry::Vacant(entry) => {
                self.local_spans.insert(ident, ident_span);
                entry.insert(MOIRValue::ValueSlice(MOIRSlice::temporary(id, 0, size.into())));
                Ok(())
            }
        }
    }

    pub fn local(
        &self,
        ident: StringId,
    ) -> Option<&MOIRValue> {
        self.locals.get(&ident)
    }

    /// Resolves the provided `ident` to a value. If the `ident does not exist in the local
    /// scope, the global scope is queried.
    pub fn resolve_value(
        &self,
        global: &ProcessorSpecificationBuilder,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<MOIRValue> {
        // TODO: Generate proper error reports
        self.locals
            .get(ident.value())
            .copied()
            .ok_or_else(|| SleighCompilerError::TODO)
            .or_else(|e| {
                global.varnode_value_by_ident(&ident).map_err(|ge| {
                    // TODO: Merge e and ge errors.
                    ge
                })
            })
    }
}
