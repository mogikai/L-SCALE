use crate::jit::api::{
    JITTaskRuntimeContext,
    RuntimeError,
};
use crate::jit::x86::{
    Space,
    X86GlobalContext,
};
use crate::tlb::{
    AccessType,
    TLB,
};
use mogi_sleigh::spec::jir::values::JIRSlice;
use std::array;
use std::mem::offset_of;
use std::ptr::NonNull;

/// This is the context of the JIT. It is stored in the RBP register, which has the alias `cpu_context`.
/// This should be kept small, only containing absolutely necessary information like the address spaces
/// or tlb pointer.
#[derive(Debug)]
pub struct X86TaskContext {
    /// Each task has its own TLB, hence this is in the task context.
    /// The TLB facilitates fast [EVAddr] to [HVAddr](mogi_utilities::addresses::HVAddr) translation.
    /// I.e. a guest address is translated to a host address where the actual data is stored.
    tlb: TLB,
    /// The address spaces for the cpu spec.
    /// TODO: Maybe make the amount of address spaces generic?
    spaces: [Space; 4],
    pc: usize,
    /// For convenience, the task context contains a pointer to the global context.
    /// The global context is however usually accessed directly in the jit code.
    global_context: NonNull<X86GlobalContext>,
    /// If a runtime error occurred, it is stored here, and the jit exits as soon
    /// as possible.
    runtime_error: Option<RuntimeError>,
}

impl X86TaskContext {
    pub fn new(global_context: NonNull<X86GlobalContext>) -> Self {
        Self {
            spaces: array::from_fn(|_| Space::new()),
            tlb: TLB::new(),
            pc: 0,
            global_context,
            runtime_error: None,
        }
    }

    pub fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    pub fn spaces_mut(&mut self) -> &mut [Space] {
        &mut self.spaces
    }

    pub fn max_spaces() -> usize {
        4
    }

    pub fn tlb_offset(access_type: AccessType) -> usize {
        match access_type {
            AccessType::Load => offset_of!(Self, tlb) + TLB::read_offset(),
            AccessType::Store => offset_of!(Self, tlb) + TLB::write_offset(),
        }
    }

    pub fn varnode_offset(slice: JIRSlice) -> usize {
        // TODO: We need to either make sure in the spec generation, that space_id is in range, or
        //       perform some kind of mapping.
        assert!(usize::from(slice.space_id) < Self::max_spaces());
        // space = spaces[varnode.space_id]
        let space_offset = offset_of!(Self, spaces) + usize::from(slice.space_id) * size_of::<Space>();
        // space[slice.offset]
        space_offset + slice.offset as usize
    }
}

impl JITTaskRuntimeContext for X86TaskContext {
    fn slice(
        &self,
        slice: JIRSlice,
    ) -> Result<&[u8], RuntimeError> {
        Ok(self.spaces[usize::from(slice.space_id)].slice(slice))
    }

    fn slice_mut(
        &mut self,
        slice: JIRSlice,
    ) -> Result<&mut [u8], RuntimeError> {
        Ok(self.spaces[usize::from(slice.space_id)].slice_mut(slice))
    }

    fn tlb(&self) -> &TLB {
        &self.tlb
    }

    fn tlb_mut(&mut self) -> &mut TLB {
        &mut self.tlb
    }

    fn set_runtime_error(
        &mut self,
        error: RuntimeError,
    ) {
        self.runtime_error = Some(error);
    }

    fn runtime_error(&self) -> Option<RuntimeError> {
        self.runtime_error
    }
}
