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
use crate::spec::moir::field::FieldId;
use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRReference,
    MOIRSlice,
};
use crate::spec::reporting::generate_identifier_exists_report;
use crate::spec::typed_index_table::TypedIndexTable;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{
    Display,
    Formatter,
};

/// A scope for locals used in constructors and macros
#[derive(Debug, Default)]
pub struct MOIRScope {
    /// Keeps track of the sizes of temporaries assigned as part of the semantic section.
    temporaries: TypedIndexTable<MOIRTemporaryId, MOIRTemporary>,
    /// A map that is used to lookup local ids by their string identifiers.
    locals: HashMap<StringId, MOIRLocalId>,
    /// Contains the actual local variables.
    slices: TypedIndexTable<MOIRLocalId, MOIRSlice>,
    /// The field sizes we have inferred. If fields resolve to varnodes, then the size can
    /// be determined by the constructor parsing before the semantic section.
    /// Otherwise, the semantic section first needs to be parsed in order to find the right
    /// size constraint. The resulting size constraint for a field will then be
    field_sizes: HashMap<FieldId, MOIRMaybeSize>,
    /// The input span of where each local was first defined. For error report generation.
    local_spans: HashMap<StringId, InputSpan>,
}

new_typed_index_table_id! { pub struct MOIRLocalId; }

impl Display for MOIRLocalId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "L{}", self.0)
    }
}

/// A temporary varnode. These have no backing varnode at this stage yet.
/// They will be mapped to concrete varnodes once the constructor is called.
#[derive(Debug)]
struct MOIRTemporary {
    pub size: MOIRMaybeSize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MOIRMaybeSize {
    Determined(MOIRSize),
    Undetermined,
}

impl MOIRMaybeSize {
    /// Returns the size if it is determined, otherwise returns the provided default.
    pub fn determined_or(
        self,
        default: MOIRSize,
    ) -> MOIRSize {
        match self {
            MOIRMaybeSize::Determined(size) => size,
            MOIRMaybeSize::Undetermined => default,
        }
    }
    pub fn ok_or<E>(
        self,
        err: E,
    ) -> Result<MOIRSize, E> {
        match self {
            MOIRMaybeSize::Determined(size) => Ok(size),
            MOIRMaybeSize::Undetermined => Err(err),
        }
    }

    pub fn ok_or_else<E, F>(
        self,
        err: F,
    ) -> Result<MOIRSize, E>
    where
        F: FnOnce() -> E,
    {
        match self {
            MOIRMaybeSize::Determined(size) => Ok(size),
            MOIRMaybeSize::Undetermined => Err(err()),
        }
    }

    /// Returns self if self is determined. Otherwise returns other.
    pub fn or(
        self,
        other: Self,
    ) -> Self {
        if matches!(self, MOIRMaybeSize::Determined(_)) {
            self
        } else {
            other
        }
    }

    pub fn is_determined(&self) -> bool {
        matches!(self, MOIRMaybeSize::Determined(_))
    }

    pub fn is_undetermined(&self) -> bool {
        matches!(self, MOIRMaybeSize::Undetermined)
    }
}

impl From<MOIRSize> for MOIRMaybeSize {
    fn from(size: MOIRSize) -> Self {
        MOIRMaybeSize::Determined(size)
    }
}

new_typed_index_table_id! { pub struct MOIRTemporaryId; }

impl MOIRScope {
    /// Declares that the local exists and points it to the corresponding slice.
    pub fn declare_local(
        &mut self,
        ident: StringId,
        value: MOIRSlice,
    ) {
        if let MOIRReference::Field(field_id) = value.reference {
            // For fields we keep track of the size that was first specified.
            // For fields with attached variables this will be the underlying
            // varnode size. For other fields, this will be the size that is
            // inferred for the first occurrence in the pcode.
            self.field_sizes.insert(field_id, value.size());
        }
        let id = self.slices.insert(value);
        self.locals.insert(ident, id);
    }

    /// Temporaries can be named or unnamed. Named temporaries are created for example by using
    /// the `local` keyword in pcode statements. Unnamed temporaries are created by the compiler
    /// for storing intermediate values when atomizing expressions.
    pub fn allocate_temporary(
        &mut self,
        size: MOIRMaybeSize,
    ) -> MOIRLocalId {
        self.slices
            .insert(MOIRSlice::temporary(self.temporaries.insert(MOIRTemporary { size }), 0, size))
    }

    /// Try to allocate a named temporary. If the temporary already exists,
    /// an error report is generated.
    pub fn allocate_named_temporary(
        &mut self,
        ident: Spanned<StringId>,
        size: MOIRMaybeSize,
    ) -> SleighCompilerResult<MOIRLocalId> {
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
                let id = self.slices.insert(MOIRSlice::temporary(id, 0, size.into()));
                entry.insert(id);
                Ok(id)
            }
        }
    }

    pub fn local(
        &self,
        ident: StringId,
    ) -> Option<&MOIRSlice> {
        let id = *self.locals.get(&ident)?;
        self.slices.get(id)
    }

    /// Returns the size of the provided local if it is determined.
    pub fn size_of_local(
        &self,
        local: MOIRLocalId,
    ) -> MOIRMaybeSize {
        self.slices
            .get(local)
            .map(MOIRSlice::size)
            .unwrap_or(MOIRMaybeSize::Undetermined)
    }

    pub fn slice_local(
        &mut self,
        local: MOIRLocalId,
        start: MOIRSize,
        end: MOIRSize,
    ) -> SleighCompilerResult<MOIRLocalId> {
        let slice = self
            .slices
            .get(local)
            .copied()
            .expect("The local id was given out by us so it must exist");
        Ok(self.slices.insert(slice.slice(start, end)?))
    }

    /// Creates a new unnamed local slice by truncating the given local.
    pub fn truncate_local(
        &mut self,
        local: MOIRLocalId,
        size: MOIRSize,
    ) -> SleighCompilerResult<MOIRLocalId> {
        let slice = self
            .slices
            .get_mut(local)
            .expect("The local id was given out by us so it must exist");
        if let MOIRReference::Field(field_id) = slice.reference {
            if slice.size().is_undetermined() {
                // This is the first time we are dealing with the size of this field.
                // Set it in the size tracking map
                let current_size = self
                    .field_sizes
                    .get_mut(&field_id)
                    .expect("The field id was given out by us so it must exist");
                *current_size = size.into();
                slice.set_size(size)?;
            }
        }
        let slice = *slice;
        Ok(self.slices.insert(slice.truncate(size)?))
    }

    /// Constrain the size of the provided local to the provided size.
    /// This will either set the size to the one provided, if it was previously undetermined,
    /// or it will return an error, if the size is different from the current size.
    pub fn constrain_local_size(
        &mut self,
        local: &mut MOIRLocal,
        size: MOIRSize,
    ) -> SleighCompilerResult {
        match local {
            MOIRLocal::Local(local) => {
                self.constrain_local_id_size(*local, size)?;
            }
            MOIRLocal::Const(val) => {
                val.constrain_size(size)?;
            }
        }
        Ok(())
    }

    pub fn constrain_local_id_size(
        &mut self,
        local: MOIRLocalId,
        size: MOIRSize,
    ) -> SleighCompilerResult {
        let slice = self
            .slices
            .get_mut(local)
            .ok_or_else(|| -> SleighCompilerError { todo!() })?;
        match slice.reference {
            MOIRReference::Temporary(temp) => {
                let temp = self
                    .temporaries
                    .get_mut(temp)
                    .expect("We gave out the id so it must exist");
                if let MOIRMaybeSize::Determined(prev_size) = temp.size {
                    if size > prev_size {
                        // The size may be smaller, since we are dealing with slices, but never larger.
                        todo!("Generate error report for size constraint violation.")
                    }
                } else {
                    // We can set the size since it was previously undetermined
                    temp.size = MOIRMaybeSize::Determined(size);
                }
            }
            MOIRReference::Field(field_id) => {
                let prev_size = self
                    .field_sizes
                    .get_mut(&field_id)
                    .expect("We gave out the id so it must exist");
                match prev_size {
                    MOIRMaybeSize::Determined(prev_size) => {
                        // If the size was previously determined, we need to make sure
                        // that the slices' size does not exceed the underlying value.
                        if size > *prev_size {
                            todo!("Generate error report for size constraint violation")
                        }
                    }
                    MOIRMaybeSize::Undetermined => {
                        // If the size was undetermined, we can simply set it now.
                        *prev_size = size.into();
                    }
                }
            }
            _ => {}
        }
        slice.set_size(size)?;
        Ok(())
    }

    /// Resolve a local to the underlying slice.
    pub fn resolve_local(
        &self,
        local: MOIRLocalId,
    ) -> SleighCompilerResult<&MOIRSlice> {
        self.slices
            .get(local)
            .ok_or(SleighCompilerError::SymbolUndeclared)
    }

    /// Resolves the provided `ident` to a local. If the `ident does not exist in the local
    /// scope, the global scope is queried.
    pub fn resolve_ident(
        &mut self,
        global: &ProcessorSpecificationBuilder,
        ident: Spanned<StringId>,
    ) -> SleighCompilerResult<MOIRLocalId> {
        // TODO: Generate proper error reports
        let id = self.locals.get(ident.value()).copied();
        match id {
            Some(id) => Ok(id),
            None => {
                // Populate a local with a slice of the global.
                let slice = global.varnode_slice_by_ident(&ident)?;
                let id = self.slices.insert(slice);
                let (ident, ident_span) = ident.into_tuple();
                self.locals.insert(ident, id);
                self.local_spans.insert(ident, ident_span);
                Ok(id)
            }
        }
    }

    /// Resolves the provided `ident` to a local. If the `ident does not exist in the local
    /// scope, the global scope is queried.
    /// Also constrains the size of the local identifier.
    pub fn resolve_ident_sized(
        &mut self,
        global: &ProcessorSpecificationBuilder,
        ident: Spanned<StringId>,
        size: MOIRSize,
    ) -> SleighCompilerResult<MOIRLocalId> {
        let local = self.resolve_ident(global, ident)?;
        let slice = self
            .slices
            .get_mut(local)
            .expect("The local id was given out by us so it must exist");
        slice.set_size(size)?;
        Ok(local)
    }
}
