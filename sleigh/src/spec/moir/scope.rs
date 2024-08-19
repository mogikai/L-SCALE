use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::lexer::InputSpan;
use crate::compiler::SleighCompilerResult;
use crate::new_symbol_id_type;
use crate::spec::builder::{
    ProcessorSpecificationBuilder,
    StringId,
};
use crate::spec::moir::varnode::{
    MOIRSlice,
    MOIRValue,
};
use crate::spec::symbol_table::MOIRSymbolTable;
use std::collections::HashMap;
use tracing::trace;

/// A scope for locals used in constructors and macros
#[derive(Debug, Default)]
pub struct MOIRScope {
    /// Keeps track of the sizes of temporaries assigned as part of the semantic section.
    temporaries: MOIRSymbolTable<MOIRTemporaryId, MOIRTemporary>,
    locals: HashMap<StringId, MOIRValue>,
    local_spans: HashMap<StringId, InputSpan>,
}

/// A temporary varnode. These have no backing varnode at this stage yet.
/// They will be mapped to concrete varnodes once the constructor is called.
#[derive(Debug)]
struct MOIRTemporary {
    pub size: u16,
}

new_symbol_id_type! { pub struct MOIRTemporaryId; }

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
    pub fn allocate_temporary(
        &mut self,
        size: u16,
    ) -> MOIRValue {
        MOIRValue::ValueSlice(MOIRSlice::temporary(self.temporaries.insert(MOIRTemporary { size }), 0, size.into()))
    }

    pub fn allocate_named_temporary(
        &mut self,
        ident: Spanned<StringId>,
        size: u16,
    ) {
        let (ident, ident_span) = ident.into_tuple();
        let id = self.temporaries.insert(MOIRTemporary { size });
        self.local_spans.insert(ident, ident_span);
        self.locals
            .insert(ident, MOIRValue::ValueSlice(MOIRSlice::temporary(id, 0, size.into())));
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
        let ident_str = global.resolve_string_id(*ident.value());
        trace!("Trying to resolve {:?} in scope {:?}", ident_str, self);
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
