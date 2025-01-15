use crate::jit::code_buffer::Code;
use dynasmrt::components::{
    LabelRegistry,
    ManagedRelocs,
    PatchLoc,
    StaticLabel,
};
use dynasmrt::relocations::{
    ImpossibleRelocation,
    Relocation,
};
use dynasmrt::{
    AssemblyOffset,
    DynamicLabel,
    DynasmApi,
    DynasmError,
    DynasmLabelApi,
    LabelKind,
    TargetKind,
};
use fnv::FnvHashMap;
use std::collections::BTreeMap;
use std::ptr::NonNull;

/// A registry of relocations and the respective labels they point towards.
#[derive(Debug, Default)]
pub struct RelocRegistry<R: Relocation> {
    static_targets: BTreeMap<usize, (PatchLoc<R>, StaticLabel)>,
    dynamic_targets: BTreeMap<usize, (PatchLoc<R>, DynamicLabel)>,
}

impl<R: Relocation> RelocRegistry<R> {
    /// Create a new, empty relocation registry.
    pub fn new() -> RelocRegistry<R> {
        RelocRegistry {
            static_targets: Default::default(),
            dynamic_targets: Default::default(),
        }
    }

    /// Add a new patch targetting a static label (either global or local).
    pub fn add_static(
        &mut self,
        label: StaticLabel,
        patchloc: PatchLoc<R>,
    ) {
        self.static_targets
            .insert(patchloc.location.0 - patchloc.field_offset as usize, (patchloc, label));
    }

    /// Add a new patch targetting the dynamic label `id`.
    pub fn add_dynamic(
        &mut self,
        id: DynamicLabel,
        patchloc: PatchLoc<R>,
    ) {
        self.dynamic_targets
            .insert(patchloc.location.0 - patchloc.field_offset as usize, (patchloc, id));
    }

    /// Return an iterator through all defined relocations targeting global labels and the labels they target.
    pub fn iter_statics(&mut self) -> impl Iterator<Item = &(PatchLoc<R>, StaticLabel)> {
        self.static_targets.values()
    }

    /// Return an iterator through all defined relocations targeting dynamic labels and the labels they target.
    pub fn iter_dynamics(&mut self) -> impl Iterator<Item = &(PatchLoc<R>, DynamicLabel)> {
        self.dynamic_targets.values()
    }

    pub fn remove_between(
        &mut self,
        start: usize,
        end: usize,
    ) {
        self.static_targets.retain(|&k, _| k >= start && k < end);
        self.dynamic_targets.retain(|&k, _| k >= start && k < end);
    }
}

/// An assembler that assembles into a `Vec<u8>`, while supporting labels. To support the different types of relocations
/// it requires a base address of the to be assembled code to be specified.
#[derive(Debug)]
pub struct MogiAssembler<R: Relocation> {
    ops: Vec<u8>,
    labels: LabelRegistry,
    /// Block global labels. These labels reference an absolute address somewhere in host memory.
    /// The label target must be located in a memory region that is continuous with the address to where
    /// this block will ultimately be written.
    global_labels: FnvHashMap<&'static str, NonNull<u8>>,
    relocs: RelocRegistry<R>,
    error: Option<DynasmError>,
}

trait CustomRelocation {
    fn global_patch(
        &self,
        buffer: &mut [u8],
        buf_addr: usize,
        global_offset: isize,
    ) -> Result<(), ImpossibleRelocation>;
}

impl<R: Relocation> CustomRelocation for PatchLoc<R> {
    fn global_patch(
        &self,
        buffer: &mut [u8],
        buf_addr: usize,
        global_offset: isize,
    ) -> Result<(), ImpossibleRelocation> {
        // Local offset of zero, but we want to globally offset
        let value = self.value(0, buf_addr) + global_offset;
        self.relocation.write_value(buffer, value)
    }
}

impl<R: Relocation + Clone> MogiAssembler<R> {
    /// Creates a new VecAssembler with the specified base address
    /// and the offset at which the assembled block will live.
    ///
    /// # Safety
    /// If the following conditions is violated, the result is Undefined Behavior:
    /// - If the offset is non-zero, then `baseaddr` must be a pointer to some allocated object,
    ///   and the entire memory range between `baseaddr` and the result must be in bounds of that
    ///   allocated object. In particular, this range must not "wrap around" the edge of the
    ///   address space.
    pub fn new() -> MogiAssembler<R> {
        MogiAssembler {
            ops: Default::default(),
            labels: Default::default(),
            global_labels: Default::default(),
            relocs: RelocRegistry::new(),
            error: None,
        }
    }

    /// Creates a new VecAssembler, with the specified base address.
    /// Preallocates `vec_capacity` bytes to the internal vector, and `label_capacity` label locations of each type.
    /// Allows the user to specify the initial capacity of the internal datastructures.
    /// `ops_capacity` is the amount of bytes preallocated for the assembling buffer.
    /// `local_labels` determines the preallocated space for local labels definitions.
    /// `global_labels` determines the preallocated space for global labels definitions.
    /// `dynamic_labels` determines the preallocated space for dynamic labels definitions.
    /// `static_references` determines the preallocated space for references to local/global labels.
    /// `dynamic_references` determines the preallocated space for references to dynamic labels.
    pub fn new_with_capacity(
        ops_capacity: usize,
        local_labels: usize,
        global_labels: usize,
        dynamic_labels: usize,
    ) -> MogiAssembler<R> {
        MogiAssembler {
            ops: Vec::with_capacity(ops_capacity),
            labels: LabelRegistry::with_capacity(local_labels, global_labels, dynamic_labels),
            global_labels: Default::default(),
            relocs: RelocRegistry::new(),
            error: None,
        }
    }

    /// Reserve capacity for at least `additional` instruction bytes to be inserted
    pub fn reserve_ops(
        &mut self,
        additional: usize,
    ) {
        self.ops.reserve(additional);
    }

    /// Create a new dynamic label ID
    pub fn new_dynamic_label(&mut self) -> DynamicLabel {
        self.labels.new_dynamic_label()
    }

    /// Fetch the size of the currently encoded instructions
    pub fn encoded_size(&self) -> usize {
        self.ops.len()
    }

    /// Writes the code from the assembly buffer to the provided code location and performs any needed relocations.
    pub unsafe fn write(
        &mut self,
        mut code: Code,
    ) -> Result<(), DynasmError> {
        if code.size() < self.ops.len() {
            todo!("Return error for insufficient size")
        }
        // If we accrued any errors while assembling before, emit them now.
        if let Some(e) = self.error.take() {
            Err(e)?
        }
        let code_ptr = code.ptr();
        let code_ptr_usize = code.ptr().addr().get();
        let buff = code.as_mut();
        buff[..self.ops.len()].copy_from_slice(&self.ops);

        // Resolve statics
        for (loc, label) in self.relocs.iter_statics() {
            let result = if let Some(target) = self.global_labels.get(label.get_name()) {
                let global_offset = target.byte_offset_from(code_ptr);
                loc.global_patch(&mut buff[loc.range(0)], code_ptr_usize, global_offset)
                    .is_err()
            } else {
                let target = self.labels.resolve_static(&label)?;
                loc.patch(&mut buff[loc.range(0)], code_ptr_usize, target.0)
                    .is_err()
            };
            if result {
                return Err(DynasmError::ImpossibleRelocation(if label.is_global() {
                    TargetKind::Global(label.get_name())
                } else {
                    TargetKind::Local(label.get_name())
                }));
            }
        }

        // Resolve dynamics
        for (loc, id) in self.relocs.iter_dynamics() {
            let target = self.labels.resolve_dynamic(*id)?;
            if loc
                .patch(&mut buff[loc.range(0)], code_ptr_usize, target.0)
                .is_err()
            {
                return Err(DynasmError::ImpossibleRelocation(TargetKind::Dynamic(*id)));
            }
        }

        Ok(())
    }

    /// Set the global label `name` to point to the provided host address.
    ///
    /// # Safety
    /// The `target` must reside in the same contiguous memory region where this block will be encoded to.
    /// If this is not the case, encoding the block will be undefined behavior.
    pub unsafe fn set_global_label(
        &mut self,
        name: &'static str,
        target: NonNull<u8>,
    ) {
        self.global_labels.insert(name, target);
    }
}

impl<R: Relocation> Extend<u8> for MogiAssembler<R> {
    fn extend<T>(
        &mut self,
        iter: T,
    ) where
        T: IntoIterator<Item = u8>,
    {
        self.ops.extend(iter)
    }
}

impl<'a, R: Relocation> Extend<&'a u8> for MogiAssembler<R> {
    fn extend<T>(
        &mut self,
        iter: T,
    ) where
        T: IntoIterator<Item = &'a u8>,
    {
        self.ops.extend(iter)
    }
}

impl<R: Relocation> DynasmApi for MogiAssembler<R> {
    fn offset(&self) -> AssemblyOffset {
        // The current assembly offset in the code buffer.
        AssemblyOffset(self.ops.len())
    }
    fn push(
        &mut self,
        byte: u8,
    ) {
        self.ops.push(byte);
    }
    fn align(
        &mut self,
        alignment: usize,
        with: u8,
    ) {
        let offset = self.offset().0 % alignment;
        if offset != 0 {
            for _ in offset..alignment {
                self.push(with);
            }
        }
    }
}

impl<R: Relocation> DynasmLabelApi for MogiAssembler<R> {
    type Relocation = R;

    fn local_label(
        &mut self,
        name: &'static str,
    ) {
        let offset = self.offset();
        self.labels.define_local(name, offset);
    }
    fn global_label(
        &mut self,
        name: &'static str,
    ) {
        if self.global_labels.contains_key(name) {
            self.error = Some(DynasmError::DuplicateLabel(LabelKind::Global(name)));
            return;
        }
        let offset = self.offset();
        if let Err(e) = self.labels.define_global(name, offset) {
            self.error = Some(e)
        }
    }
    fn dynamic_label(
        &mut self,
        id: DynamicLabel,
    ) {
        let offset = self.offset();
        if let Err(e) = self.labels.define_dynamic(id, offset) {
            self.error = Some(e)
        }
    }
    fn forward_relocation(
        &mut self,
        name: &'static str,
        target_offset: isize,
        field_offset: u8,
        ref_offset: u8,
        kind: R,
    ) {
        let location = self.offset();
        let label = match self.labels.place_local_reference(name) {
            Some(label) => label.next(),
            None => StaticLabel::first(name),
        };
        self.relocs
            .add_static(label, PatchLoc::new(location, target_offset, field_offset, ref_offset, kind));
    }
    fn backward_relocation(
        &mut self,
        name: &'static str,
        target_offset: isize,
        field_offset: u8,
        ref_offset: u8,
        kind: R,
    ) {
        let location = self.offset();
        let label = match self.labels.place_local_reference(name) {
            Some(label) => label,
            None => {
                self.error = Some(DynasmError::UnknownLabel(LabelKind::Local(name)));
                return;
            }
        };
        self.relocs
            .add_static(label, PatchLoc::new(location, target_offset, field_offset, ref_offset, kind));
    }
    fn global_relocation(
        &mut self,
        name: &'static str,
        target_offset: isize,
        field_offset: u8,
        ref_offset: u8,
        kind: R,
    ) {
        let location = self.offset();
        let label = StaticLabel::global(name);
        self.relocs
            .add_static(label, PatchLoc::new(location, target_offset, field_offset, ref_offset, kind));
    }
    fn dynamic_relocation(
        &mut self,
        id: DynamicLabel,
        target_offset: isize,
        field_offset: u8,
        ref_offset: u8,
        kind: R,
    ) {
        let location = self.offset();
        self.relocs
            .add_dynamic(id, PatchLoc::new(location, target_offset, field_offset, ref_offset, kind));
    }
    fn bare_relocation(
        &mut self,
        _target: usize,
        _field_offset: u8,
        _ref_offset: u8,
        _kind: R,
    ) {
        panic!("bare_relocation is not supported by MogiAssembler");
        // let location = self.offset();
        // let loc = PatchLoc::new(location, 0, field_offset, ref_offset, kind);
        // let buf = &mut self.ops[loc.range(0)];
        // if loc
        //     .patch(buf, 0, target)
        //     .is_err()
        // {
        //     self.error = Some(DynasmError::ImpossibleRelocation(TargetKind::Extern(target)))
        // }
    }
}
