use crate::compiler::InternalCompilerResult;
use crate::compiler::error::{
    DuplicatedIdentifier,
    InternalError,
    InternalErrorType,
};
use crate::spec::builder::ProcessorSpecificationBuilder;
use crate::spec::moir::code::{
    MOIRInstruction,
    moir_instruction,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::{
    MOIRLocal,
    MOIRReference,
    MOIRSlice,
};
use mogi_sleigh_parser::lexer::InputSpan;
use mogi_sleigh_parser::{
    ParserContext,
    SleighInterner,
    Spanned,
    StringId,
};
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use mogi_utilities::{
    new_map_key,
    new_map_key_16,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::collections::hash_map::Entry;
use std::collections::{
    HashMap,
    HashSet,
};
use std::fmt::{
    Debug,
    Display,
    Formatter,
    Write,
};

/// A scope for locals and labels used in constructors and macros
#[derive(Clone, Serialize, Deserialize)]
pub struct MOIRScope {
    /// A hashmap to keep track of label ids and the location of their first definition in the span.
    labels_definitions: HashMap<StringId, Spanned<LabelId>>,
    /// A map that is used to lookup local ids by their string identifiers.
    locals: HashMap<StringId, MOIRLocalId>,
    /// The input span of where each local was first defined. For error report generation.
    local_spans: HashMap<StringId, InputSpan>,
    /// Labels can be used ahead of their definition. Their usage declares them, and they can
    /// be defined later on. If no definition was found after processing the entire semantic
    /// section, this will produce an error.
    labels: PrimaryMap<LabelId, Option<InstructionId>>,
    /// Keeps track of the sizes of temporaries assigned as part of the semantic section.
    temporaries: PrimaryMap<MOIRTemporaryId, Option<MOIRTemporary>>,
    /// Contains the actual local variables.
    slices: PrimaryMap<MOIRLocalId, MOIRSlice>,

    /// The field sizes we have inferred. If fields resolve to varnodes, then the size can
    /// be determined by the constructor parsing before the semantic section.
    /// Otherwise, the semantic section first needs to be parsed in order to find the right
    /// size constraint. The resulting size constraint for a field will then be
    field_sizes: SecondaryMap<FieldId, MOIRMaybeSize>,
    /// A map that is used to lookup local ids by field ids.
    field_to_local: SecondaryMap<FieldId, Option<MOIRLocalId>>,

    /// A helper set to keep track of which local variable is actually used.
    /// Unused variables can be removed from this set.
    /// The set is used to determine how many actual temporary varnodes need to be allocated later on.
    used_locals: HashSet<MOIRLocalId>,
    /// This map keeps track of where the subtable build instruction is
    /// By default, subtables are built implicitly at the beginning of the
    /// instruction list. If an explicit build instruction occurs later,
    /// the implicit build is replaced by a `nop` and the subtable is marked
    /// as explicitly built. A second explicit build is not allowed.
    subtable_builds: HashMap<TableId, SubTableBuild>,

    /// These locals are explicitly set from outside so we need to store their id.
    inst_start: MOIRLocalId,
    inst_next: MOIRLocalId,
    inst_next2: MOIRLocalId,
}

new_map_key_16! {
    pub struct MOIRLocalId; "L";
}
new_map_key! { pub struct LabelId; "LBL"; }

/// InstructionId is simply an index into the instruction list.
pub type InstructionId = usize;

/// A small enum to keep track of whether the subtable is implicitly or explicitly built.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
enum SubTableBuild {
    Implicit(InstructionId),
    Explicit,
}

/// A temporary varnode. These have no backing varnode at this stage yet.
/// They will be mapped to concrete varnodes once the constructor is called.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub(crate) struct MOIRTemporary {
    pub size: MOIRMaybeSize,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MOIRMaybeSize {
    Determined(MOIRSize),
    #[default]
    Undetermined,
}

impl PartialEq<MOIRSize> for MOIRMaybeSize {
    fn eq(
        &self,
        other: &MOIRSize,
    ) -> bool {
        self.eq(&MOIRMaybeSize::Determined(*other))
    }
}

impl Display for MOIRMaybeSize {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRMaybeSize::Determined(size) => write!(f, "{size}"),
            MOIRMaybeSize::Undetermined => write!(f, "UD"),
        }
    }
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

    pub fn or_else<F>(
        self,
        f: F,
    ) -> Self
    where
        F: FnOnce() -> Self,
    {
        if matches!(self, MOIRMaybeSize::Determined(_)) {
            self
        } else {
            f()
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

impl From<Option<MOIRSize>> for MOIRMaybeSize {
    fn from(value: Option<MOIRSize>) -> Self {
        match value {
            None => MOIRMaybeSize::Undetermined,
            Some(size) => MOIRMaybeSize::Determined(size),
        }
    }
}

new_map_key_16! { pub struct MOIRTemporaryId; "T"; }
new_map_key! { pub struct ActionValueId; "AV"; }

impl MOIRScope {
    pub fn new(
        inst_start: StringId,
        inst_next: StringId,
        inst_next2: StringId,
        addr_size: MOIRSize,
    ) -> Self {
        // these predefined consts have a known size
        let s = MOIRMaybeSize::Determined(addr_size);

        let mut temporaries = Default::default();
        let mut locals = Default::default();
        let mut local_spans = Default::default();
        let mut slices = Default::default();

        let inst_start = internal_allocate_named_const(
            &mut temporaries,
            &mut locals,
            &mut local_spans,
            &mut slices,
            Spanned::new(inst_start, InputSpan::default()),
            s,
        )
        .expect("This can't fail at this point");
        let inst_next = internal_allocate_named_const(
            &mut temporaries,
            &mut locals,
            &mut local_spans,
            &mut slices,
            Spanned::new(inst_next, InputSpan::default()),
            s,
        )
        .expect("This can't fail at this point");
        let inst_next2 = internal_allocate_named_const(
            &mut temporaries,
            &mut locals,
            &mut local_spans,
            &mut slices,
            Spanned::new(inst_next2, InputSpan::default()),
            s,
        )
        .expect("This can't fail at this point");
        Self {
            labels_definitions: Default::default(),
            locals,
            local_spans,
            used_locals: Default::default(),
            slices,
            field_sizes: Default::default(),
            temporaries,
            inst_start,
            inst_next,
            inst_next2,
            labels: Default::default(),
            field_to_local: Default::default(),
            subtable_builds: Default::default(),
        }
    }

    /// Clears the namespace before calling the provided function and afterward restores it to its original state.
    /// This allows atomization of macro calls, because they have their own name resolution, but use the same
    /// scope of locals, just with different name mappings.
    pub fn with_empty_namespace<F, R>(
        &mut self,
        f: F,
    ) -> InternalCompilerResult<R>
    where
        F: FnOnce(&mut Self) -> InternalCompilerResult<R>,
    {
        let label_definitions = std::mem::take(&mut self.labels_definitions);
        let locals = std::mem::take(&mut self.locals);
        let local_spans = std::mem::take(&mut self.local_spans);
        let result = f(self);
        self.local_spans = local_spans;
        self.locals = locals;
        self.labels_definitions = label_definitions;
        result
    }

    pub(crate) fn iter_used_temps(&self) -> impl Iterator<Item = (MOIRTemporaryId, MOIRTemporary)> {
        self.used_locals.iter().copied().filter_map(|local_id| {
            match self
                .slices
                .get(local_id)
                .expect("The local must exist. Otherwise we have a logic error in the Scope struct.")
                .reference
            {
                MOIRReference::Temporary(temp_id) => Some((
                    temp_id,
                    self.temporaries[temp_id]
                        .expect("This must exist. Otherwise we have a logic error in the Scope struct"),
                )),
                _ => None,
            }
        })
    }

    pub fn mark_local_as_used(
        &mut self,
        local: MOIRLocal,
    ) -> InternalCompilerResult {
        match local {
            MOIRLocal::Local(id) => {
                self.used_locals.insert(id);
            }
            MOIRLocal::Const(_) => {
                // For constants, we don't need to track anything
            }
        }
        Ok(())
    }

    /// Declares a name for an existing local. If the name already exists, an error is returned.
    /// This is usually used in conjunction with [with_empty_namespace] in order to create an
    /// aliased scope for macros to work in.
    pub fn declare_local_reference(
        &mut self,
        ident: Spanned<StringId>,
        local: MOIRLocalId,
    ) -> InternalCompilerResult {
        let (ident, span) = ident.into_tuple();
        match self.locals.entry(ident) {
            Entry::Occupied(_) => {
                Err(InternalError::with_span(InternalErrorType::RedefinedSymbol, "dupe", span.clone()))?
            }
            Entry::Vacant(entry) => entry.insert(local),
        };
        self.local_spans.insert(ident, span);
        Ok(())
    }

    /// Declares that the local exists and points it to the corresponding slice.
    pub fn declare_local(
        &mut self,
        ident: StringId,
        value: MOIRSlice,
    ) -> MOIRLocalId {
        let id = self.slices.insert(value);
        if let MOIRReference::Field(field_id) = value.reference {
            // For fields we keep track of the size that was first specified.
            // For fields with attached variables this will be the underlying
            // varnode size. For other fields, this will be the size that is
            // inferred for the first occurrence in the pcode.
            self.field_sizes[field_id] = value.size();

            // We also store a mapping from field id to local id for insertion
            // later on.
            self.field_to_local[field_id] = Some(id);
        }

        self.locals.insert(ident, id);
        id
    }

    /// Marks subtables as implicitly built and stores the instruction ids where the builds are performed.
    pub fn make_implicit_builds(
        &mut self,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> InternalCompilerResult {
        for subtable in self
            .slices
            .items()
            .filter_map(|(_, slice)| match slice.reference {
                MOIRReference::SubTable(id) => Some(id),
                _ => None,
            })
        {
            // We insert an implicit build instruction for each subtable at the beginning.
            // This results in a recursive depth first build of subtables.
            // The order is not determined.
            // Explicit build instructions later on should remove the implicit build instruction.
            // The instruction is simply replaced with a `nop` instruction which is omitted when
            // transforming to JIR.
            let index = instructions.len();
            instructions.push(moir_instruction!(build subtable));
            // Save the index of the build instruction
            match self.subtable_builds.entry(subtable) {
                Entry::Occupied(_) => todo!("Generate error: Cannot implicitly build the same table twice"),
                Entry::Vacant(e) => e.insert(SubTableBuild::Implicit(index)),
            };
        }
        Ok(())
    }

    pub fn mark_explicit_build(
        &mut self,
        table_id: TableId,
        instructions: &mut Vec<MOIRInstruction>,
    ) -> InternalCompilerResult {
        match self.subtable_builds.entry(table_id) {
            Entry::Occupied(mut e) => {
                match e.get() {
                    SubTableBuild::Implicit(inst) => instructions[*inst] = moir_instruction!(nop),
                    SubTableBuild::Explicit => todo!("Generate error for duplicate explicit build instructions"),
                };
                e.insert(SubTableBuild::Explicit);
            }
            Entry::Vacant(_) => panic!("Logic error. All subtables need to have been marked as implicitly built"),
        }
        Ok(())
    }

    /// Temporaries can be named or unnamed. Named temporaries are created for example by using
    /// the `local` keyword in pcode statements. Unnamed temporaries are created by the compiler
    /// for storing intermediate values when atomizing expressions.
    pub fn allocate_temporary(
        &mut self,
        size: impl Into<MOIRMaybeSize>,
    ) -> MOIRLocalId {
        let size = size.into();
        self.slices
            .insert(MOIRSlice::temporary(self.temporaries.insert(Some(MOIRTemporary { size })), 0, size))
    }

    /// Named consts are treated in a similar fashing as named temporaries, but no backing varnode
    /// is allocated and they don't have a size.
    pub fn allocate_named_const(
        &mut self,
        ident: Spanned<StringId>,
        size: MOIRMaybeSize,
    ) -> InternalCompilerResult<MOIRLocalId> {
        internal_allocate_named_const(
            &mut self.temporaries,
            &mut self.locals,
            &mut self.local_spans,
            &mut self.slices,
            ident,
            size,
        )
    }

    /// Try to allocate a named temporary. If the temporary already exists,
    /// an error report is generated.
    pub fn allocate_named_temporary(
        &mut self,
        ident: Spanned<StringId>,
        size: MOIRMaybeSize,
    ) -> InternalCompilerResult<MOIRLocalId> {
        let (ident, ident_span) = ident.into_tuple();
        let id = self.temporaries.insert(Some(MOIRTemporary { size }));
        match self.locals.entry(ident) {
            Entry::Occupied(_) => Err(InternalError::with_spans(
                DuplicatedIdentifier { name: "".to_string() }.into(),
                HashMap::from([
                    (
                        "first",
                        self.local_spans
                            .get(&ident)
                            .expect("This must exist, since the locals entry is occupied")
                            .clone(),
                    ),
                    ("second", ident_span),
                ]),
            ))?,
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
        self.slices.get(self.local_id_by_ident(ident)?)
    }

    /// Return the size of the field specified by `field_id` or an error if no
    /// size is stored.
    pub fn field_size(
        &self,
        field_id: FieldId,
    ) -> InternalCompilerResult<MOIRMaybeSize> {
        self.field_sizes
            .get(&field_id)
            .ok_or_else(|| -> InternalError { todo!("Generate error report for missing field size") })
            .copied()
    }

    pub fn local_id_by_field_id(
        &self,
        field_id: FieldId,
    ) -> Option<MOIRLocalId> {
        self.field_to_local[field_id]
    }

    pub fn local_id_by_ident(
        &self,
        ident: StringId,
    ) -> Option<MOIRLocalId> {
        self.locals.get(&ident).copied()
    }

    pub fn inst_start(&self) -> MOIRLocalId {
        self.inst_start
    }

    pub fn inst_next(&self) -> MOIRLocalId {
        self.inst_next
    }

    pub fn inst_next2(&self) -> MOIRLocalId {
        self.inst_next2
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
    ) -> InternalCompilerResult<MOIRLocalId> {
        let slice = self
            .slices
            .get(local)
            .copied()
            .expect("The local id was given out by us so it must exist");
        Ok(self.slices.insert(slice.slice(start, end)?))
    }

    /// Creates a new unnamed local slice by truncating the given local.
    /// If the size of the local is already the same as the specified size,
    /// then this operation does nothing, and simply returns the existing
    /// local.
    pub fn truncate_local(
        &mut self,
        local: MOIRLocalId,
        size: MOIRSize,
    ) -> InternalCompilerResult<MOIRLocalId> {
        let slice = self
            .slices
            .get_mut(local)
            .expect("The local id was given out by us so it must exist");
        if slice.size() == size {
            // We can return early, since the size is already determined and the same as the current slice.
            return Ok(local);
        }
        if let MOIRReference::Field(field_id) = slice.reference {
            if slice.size().is_undetermined() {
                // This is the first time we are dealing with the size of this field.
                // Set it in the size tracking map
                self.field_sizes[field_id] = size.into();
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
    ) -> InternalCompilerResult {
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
    ) -> InternalCompilerResult {
        let slice = self
            .slices
            .get_mut(local)
            .ok_or_else(|| -> InternalError { todo!() })?;
        match slice.reference {
            MOIRReference::Temporary(temp) => {
                let temp = self.temporaries[temp]
                    .as_mut()
                    .expect("The temporary must exist");
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
                let prev_size = &mut self.field_sizes[field_id];
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
    ) -> InternalCompilerResult<&MOIRSlice> {
        self.slices
            .get(local)
            .ok_or(InternalErrorType::UndeclaredSymbol.into())
    }

    /// Defines the label to point to a specific instruction.
    /// This definition can occur only once and will return an error otherwise.
    pub fn define_label(
        &mut self,
        label: Spanned<StringId>,
        id: InstructionId,
    ) -> InternalCompilerResult<(LabelId, InstructionId)> {
        let label_id = self.resolve_label(label)?;
        let instruction_id = &mut self.labels[label_id];
        if instruction_id.is_some() {
            todo!("Generate error for duplicate label definition")
        } else {
            *instruction_id = Some(id);
        }
        Ok((label_id, id))
    }

    /// Resolve the label to its LabelId. If this is the first time this label is encountered, an id is generated.
    /// If the label is not defined after processing the semantic section, this will result in an error being
    /// generated.
    pub fn resolve_label(
        &mut self,
        label: Spanned<StringId>,
    ) -> InternalCompilerResult<LabelId> {
        match self.labels_definitions.entry(*label.value()) {
            Entry::Occupied(entry) => Ok(*entry.get().value()),
            Entry::Vacant(entry) => {
                // Insert None for the location since we don't know this during lookup.
                // A later call to [define_label] has to occur, in order to avoid an error.
                let id = self.labels.insert(None);
                // We can then return the newly generated label id.
                Ok(*entry.insert(Spanned::new(id, label.into_span())).value())
            }
        }
    }

    /// Resolves the provided `ident` to a local. If the `ident does not exist in the local
    /// scope, the global scope is queried.
    pub fn resolve_ident(
        &mut self,
        global: &ProcessorSpecificationBuilder,
        ident: Spanned<StringId>,
    ) -> InternalCompilerResult<MOIRLocalId> {
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
    ) -> InternalCompilerResult<MOIRLocalId> {
        let local = self.resolve_ident(global, ident)?;
        let slice = self
            .slices
            .get_mut(local)
            .expect("The local id was given out by us so it must exist");
        slice.set_size(size)?;
        Ok(local)
    }

    pub fn get_used_fields(&self) -> impl Iterator<Item = (FieldId, MOIRLocalId)> {
        self.used_locals.iter().filter_map(|local_id| {
            let slice = self
                .slices
                .get(*local_id)
                .expect("Generate error report for missing slice");
            match slice.reference {
                MOIRReference::Field(field_id) => Some((field_id, *local_id)),
                _ => None,
            }
        })
    }

    pub fn get_used_field(
        &self,
        field_id: FieldId,
    ) -> Option<MOIRLocalId> {
        self.used_locals.iter().find_map(|local_id| {
            let slice = self
                .slices
                .get(*local_id)
                .expect("Generate error report for missing slice");
            match slice.reference {
                MOIRReference::Field(ref_field_id) => {
                    if ref_field_id == field_id {
                        Some(*local_id)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
    }

    pub fn detailed_scope_display(
        &self,
        interner: &SleighInterner,
    ) -> String {
        let mut already_printed = HashMap::new();
        for (ident, local_id) in self.locals.iter() {
            let slice = self.slices.get(*local_id).expect("This must exist");
            if self.used_locals.contains(local_id) {
                let str = interner.string(*ident).expect("This must exist");
                let str = format!("{: <18} {}", format!("{: <14}({}):", str, local_id), slice);
                already_printed.insert(*local_id, str);
            }
        }
        let mut out = String::new();
        for (id, slice) in self.slices.items() {
            if self.used_locals.contains(&id) {
                if let Some(str) = already_printed.get(&id) {
                    writeln!(out, "{str}").expect("This should not error");
                } else {
                    writeln!(out, "{: <4} {}", format!("{: <14}({}):", "<unnamed>", id), slice)
                        .expect("This should not error");
                }
            }
        }
        out
    }
}

fn internal_allocate_named_const(
    temporaries: &mut PrimaryMap<MOIRTemporaryId, Option<MOIRTemporary>>,
    locals: &mut HashMap<StringId, MOIRLocalId>,
    local_spans: &mut HashMap<StringId, InputSpan>,
    slices: &mut PrimaryMap<MOIRLocalId, MOIRSlice>,
    ident: Spanned<StringId>,
    size: MOIRMaybeSize,
) -> InternalCompilerResult<MOIRLocalId> {
    let (ident, ident_span) = ident.into_tuple();
    let id = temporaries.insert(Some(MOIRTemporary { size }));
    match locals.entry(ident) {
        Entry::Occupied(_) => Err(InternalError::with_spans(
            DuplicatedIdentifier { name: "".to_string() }.into(),
            HashMap::from([
                (
                    "first",
                    local_spans
                        .get(&ident)
                        .expect("This must exist, since the locals entry is occupied")
                        .clone(),
                ),
                ("second", ident_span.clone()),
            ]),
        )),
        Entry::Vacant(entry) => {
            local_spans.insert(ident, ident_span);
            let id = slices.insert(MOIRSlice::named_const(id, size));
            entry.insert(id);
            Ok(id)
        }
    }
}

impl Display for MOIRScope {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        for (id, slice) in self.slices.items() {
            if self.used_locals.contains(&id) {
                writeln!(f, "{: <4} {}", format!("{}:", id), slice)?;
            }
        }
        Ok(())
    }
}

impl Debug for MOIRScope {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        for (id, slice) in self.slices.items() {
            writeln!(f, "{: <4} {}", format!("{}:", id), slice)?;
        }
        Ok(())
    }
}
