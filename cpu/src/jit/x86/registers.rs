use crate::jit::api::{
    JITResult,
    JitError,
};
use crate::jit::trace_jit;
use crate::jit::x86::regalloc::{
    RegAllocHint,
    RegAllocIntention,
    SpillBehavior,
};
use bitvec::array::BitArray;
use mogi_sleigh::spec::jir::values::JIRSlice;
use num_enum::{
    IntoPrimitive,
    TryFromPrimitive,
};
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::marker::PhantomData;
use std::ops::{
    Index,
    IndexMut,
};
use std::{
    array,
    mem,
};

pub trait FlushEmitter<AssemblerT>: Fn(&mut AssemblerT, GPRegister, JIRSlice) -> JITResult {}
impl<AssemblerT, T> FlushEmitter<AssemblerT> for T where T: Fn(&mut AssemblerT, GPRegister, JIRSlice) -> JITResult {}

pub trait Register: TryFrom<Self::Primitive, Error: Debug> {
    type Primitive: TryFrom<usize, Error: Debug>;
    const NUMBER: usize;
    /// Return the initial allocation for the specified register.
    fn initial_allocation(&self) -> RegAlloc;
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// Represents all general purpose registers available on x64.
/// The size of the register is not encoded here. This enum only
/// serves as a selector for the register and not for its size.
pub enum GPRegister {
    AX  = 0,
    CX  = 1,
    DX  = 2,
    BX  = 3,
    SP  = 4,
    BP  = 5,
    SI  = 6,
    DI  = 7,
    R8  = 8,
    R9  = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

impl Register for GPRegister {
    type Primitive = u8;
    const NUMBER: usize = 16;

    fn initial_allocation(&self) -> RegAlloc {
        let ty = match self {
            // we reserve RAX as temporary since there are some instructions that can only use RAX
            GPRegister::AX => RegAllocType::Reserved,
            // RBP is reserved for the cpu_context
            GPRegister::BP => RegAllocType::Reserved,
            // We don't want to mess with the stack pointer.
            // TODO: It may make sense to keep the emulated stack pointer loaded in some register.
            //       This might even be the host SP register, but we will have to save the value somewhere.
            //       Maybe it even makes sense to provide an api where one can specify target registers
            //       that should be loaded into fixed host registers upon entering the jit. (SP?, FLAGS?)
            GPRegister::SP => RegAllocType::Reserved,
            _ => RegAllocType::Free,
        };
        RegAlloc { ty, version: 0 }
    }
}

impl From<GPRegister> for usize {
    fn from(value: GPRegister) -> Self {
        usize::from(u8::from(value))
    }
}

#[derive(Debug, Copy, Clone, TryFromPrimitive, IntoPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// This enum represents the available XMM, YMM, and ZMM Registers. (YMM and ZMM are extensions of XMM)
/// These registers are primarily used for floating point arithmetic and simd operations.
enum XMMRegister {
    XMM0  = 0,
    XMM1  = 1,
    XMM2  = 2,
    XMM3  = 3,
    XMM4  = 4,
    XMM5  = 5,
    XMM6  = 6,
    XMM7  = 7,
    XMM8  = 8,
    XMM9  = 9,
    XMM10 = 10,
    XMM11 = 11,
    XMM12 = 12,
    XMM13 = 13,
    XMM14 = 14,
    XMM15 = 15,
}

impl From<XMMRegister> for usize {
    fn from(value: XMMRegister) -> Self {
        usize::from(u8::from(value))
    }
}

impl Register for XMMRegister {
    type Primitive = u8;
    const NUMBER: usize = 16;

    fn initial_allocation(&self) -> RegAlloc {
        let ty = match self {
            _ => RegAllocType::Free,
        };
        RegAlloc { ty, version: 0 }
    }
}

#[derive(Debug)]
pub struct RegisterAllocations<T, const N: usize>([RegAlloc; N], PhantomData<T>)
where
    T: Register;

impl<T, const N: usize> Display for RegisterAllocations<T, N>
where
    T: Register,
{
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "")?;
        for (i, alloc) in self.0.iter().enumerate() {
            write!(f, "v{:03}:{:?}", alloc.version, alloc.ty)?;
            if i < self.0.len() - 1 {
                write!(f, "\n")?;
            }
        }
        Ok(())
    }
}

impl<T, const N: usize> Default for RegisterAllocations<T, N>
where
    T: Register,
{
    fn default() -> Self {
        Self(
            array::from_fn(|i| {
                T::try_from(T::Primitive::try_from(i).expect("T::Primitive must fit into N"))
                    .expect("T::Primitive must fit into N")
                    .initial_allocation()
            }),
            Default::default(),
        )
    }
}

impl<T, const N: usize> RegisterAllocations<T, N>
where
    T: Register,
{
    pub fn iter(&self) -> impl Iterator<Item = (GPRegister, &RegAlloc)> {
        self.0.iter().enumerate().map(|(i, entry)| {
            (GPRegister::try_from(i as u8).expect("We iterate over the GP ids, hence this must not error"), entry)
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (GPRegister, &mut RegAlloc)> {
        self.0.iter_mut().enumerate().map(|(i, entry)| {
            (GPRegister::try_from(i as u8).expect("We iterate over the GP ids, hence this must not error"), entry)
        })
    }
}

impl<T, const N: usize> Index<GPRegister> for RegisterAllocations<T, N>
where
    T: Register,
{
    type Output = RegAlloc;

    fn index(
        &self,
        index: GPRegister,
    ) -> &Self::Output {
        &self.0[index as usize]
    }
}

impl<T, const N: usize> IndexMut<GPRegister> for RegisterAllocations<T, N>
where
    T: Register,
{
    fn index_mut(
        &mut self,
        index: GPRegister,
    ) -> &mut Self::Output {
        &mut self.0[index as usize]
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RegAlloc {
    pub ty: RegAllocType,
    /// Keeps track of when this allocation was performed
    /// Allocations performed for the current instruction may not be flushed.
    version: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum RegAllocType {
    Free,
    Reserved,
    /// The slot is occupied by a temporary. These can simply be discarded if no longer needed
    OccupiedTemp,
    /// The slot is occupied by a slice. The slice was not modified and thus can be discarded.
    Occupied(JIRSlice),
    /// The slot is occupied by a slice. The slice needs to be flushed to memory if the slot is cleared.
    OccupiedDirty(JIRSlice),
}

/// This struct is used to keep track of which slice is loaded in what register.
/// TODO: This most likely needs to be optimized for size.
#[derive(Debug, Default)]
pub struct RegisterTracker {
    // TODO: Use GPRegister::NUMBER once generic_const_exprs is stable.
    pub gp: RegisterAllocations<GPRegister, 16>,
    xmm: RegisterAllocations<XMMRegister, 16>,
    /// The allocation version. Anything with this current version may not be flushed when allocating.
    /// The version can also be used to determine how far into the past a register was allocated.
    version: usize,
}

impl RegisterTracker {
    /// Resets all GP register slots to empty and returns the array of slices that were previously associated with
    /// the registers. This can be used to emit the flush instructions.
    /// This also resets the allocation version to zero.
    pub fn reset_gp(&mut self) -> RegisterAllocations<GPRegister, 16> {
        self.version = 0;
        mem::take(&mut self.gp)
    }

    /// New allocations will have a higher version that any previous allocation.
    /// This effectively marks all registers with lower versions as potential flush
    /// candidates.
    pub fn new_alloc_version(&mut self) {
        self.version += 1;
    }

    pub fn slice_loaded_in_gpr(
        &self,
        slice: JIRSlice,
        gpr: GPRegister,
    ) -> bool {
        match &self.gp[gpr].ty {
            RegAllocType::Occupied(loaded_slice) | RegAllocType::OccupiedDirty(loaded_slice) => loaded_slice == &slice,
            _ => false,
        }
    }

    /// Allocate a gp register for use as a temporary or slice, depending on `alloc_type`.
    /// If a slice is already loaded, the corresponding GPR is returned.
    /// If a slice has not been loaded previously, a free GPR is returned, but no load is performed.
    /// The [RegisterLoadInfo] indicates, whether the GPR is loaded or not.
    /// Unloaded GPRs can be used if the slice appears as output, since we will overwrite the value anyway.
    pub fn allocate_gpr<AssemblerT, FlushEmitterT>(
        &mut self,
        assembler: &mut AssemblerT,
        emit_flush: FlushEmitterT,
        hint: RegAllocHint,
        alloc_intention: RegAllocIntention,
        spill_behavior: SpillBehavior,
    ) -> JITResult<RegisterLoadInfo<GPRegister>>
    where
        FlushEmitterT: FlushEmitter<AssemblerT>,
    {
        let result = match self.find_existing(alloc_intention, hint) {
            Ok(load_info) => Ok(load_info),
            Err((free_gpr, existing_gpr)) => {
                if let Some(free_gpr) = free_gpr {
                    self.perform_alloc_gp(hint, alloc_intention, existing_gpr, free_gpr)
                } else {
                    match spill_behavior {
                        SpillBehavior::Spill => {
                            // TODO: In some cases we dont really need to spill here. Probably need to rework the alloc function
                            let free_gpr = self.spill_one_register(assembler, emit_flush)?;
                            self.perform_alloc_gp(hint, alloc_intention, existing_gpr, free_gpr)
                        }
                        SpillBehavior::Fail => Err(JitError::NoRegistersAvailable),
                    }
                }
            }
        }?;
        let gpr = match result {
            RegisterLoadInfo::Unloaded(reg) => reg,
            RegisterLoadInfo::Loaded(reg) => reg,
            RegisterLoadInfo::NeedRelocation { to, .. } => to,
        };
        // TODO: Make this pretty since it is fucking ugly.
        // Update the version of the gp we allocated.
        self.gp[gpr].version = self.version;
        Ok(result)
    }

    /// Tries to look for an existing register for the
    pub fn find_existing(
        &self,
        alloc_intention: RegAllocIntention,
        hint: RegAllocHint,
    ) -> Result<RegisterLoadInfo<GPRegister>, (Option<GPRegister>, Option<GPRegister>)> {
        // TODO: This whole function is spaghetti and needs to be reworked.
        let mut free_gpr = if let RegAllocHint::Reallocate(hint) = hint {
            // If the alloc hint is set to realloc, we simply set the provided register as free.
            // The caller needs to make sure that it is actually free.
            Some(hint)
        } else {
            None
        };
        let mut existing_gpr = None;
        // TODO: We need to check if two slices refer to the same memory.
        //       This needs to be done because there might be a slice to e.g. EAX loaded, but another
        //       instruction refers to RAX. The RAX load then needs to be performed from the loaded
        //       EAX, instead of from the space, or it needs to be flushed first.
        // Search for existing register, and otherwise take the first available GP register.
        for (register, entry) in self.gp.iter() {
            match &entry.ty {
                RegAllocType::Free => {
                    if let RegAllocHint::Allocate(hint) = hint
                        && hint == register
                    {
                        // If the hinted register is free, we set it as the chosen register
                        // even if another free register was already found.
                        free_gpr = Some(register);
                    }
                    if free_gpr.is_none() {
                        // Save the first free register
                        free_gpr = Some(register)
                    }
                }
                RegAllocType::Reserved | RegAllocType::OccupiedTemp => {}
                RegAllocType::Occupied(current) | RegAllocType::OccupiedDirty(current) => match alloc_intention {
                    RegAllocIntention::Slice(slice) if current.varnode_id == slice.varnode_id => {
                        if current.size < slice.size() || current.offset != slice.offset {
                            todo!("Aliasing varnodes are currently not supported yet")
                        }
                        if let RegAllocHint::Reallocate(_) = hint {
                            // We found an existing register and can now break to reallocate existing to hint.
                            existing_gpr = Some(register);
                            break;
                        } else {
                            // We found an existing register and do not want to move the slice, so we just return.
                            return Ok(RegisterLoadInfo::Loaded(register));
                        }
                    }
                    RegAllocIntention::OutSlice(slice) if current.varnode_id == slice.varnode_id => {
                        if current.size < slice.size() || current.offset != slice.offset {
                            todo!("Aliasing varnodes are currently not supported yet")
                        }
                        existing_gpr = Some(register);
                        if matches!(hint, RegAllocHint::Reallocate(_)) {
                            // When moving, we can now break since we have all needed information.
                            break;
                        };
                    }
                    RegAllocIntention::Realloc(slice) if current.varnode_id == slice.varnode_id => {
                        if current.size < slice.size() || current.offset != slice.offset {
                            todo!("Aliasing varnodes are currently not supported yet")
                        }
                        existing_gpr = Some(register);
                    }
                    _ => {}
                },
            }
        }
        Err((free_gpr, existing_gpr))
    }

    pub fn perform_alloc_gp(
        &mut self,
        hint: RegAllocHint,
        alloc_type: RegAllocIntention,
        existing_gpr: Option<GPRegister>,
        free_gpr: GPRegister,
    ) -> JITResult<RegisterLoadInfo<GPRegister>> {
        match alloc_type {
            RegAllocIntention::Temp => {
                self.gp[free_gpr].ty = RegAllocType::OccupiedTemp;
                Ok(RegisterLoadInfo::Loaded(free_gpr))
            }
            RegAllocIntention::Slice(slice) => {
                let Some(existing_gpr) = existing_gpr else {
                    // The out slice is not loaded yet, we need to load it now.
                    self.gp[free_gpr].ty = RegAllocType::Occupied(slice);
                    return Ok(RegisterLoadInfo::Unloaded(free_gpr));
                };
                if matches!(hint, RegAllocHint::Reallocate(_)) {
                    // The out slice is already loaded. Move the slice if necessary
                    self.realloc_gp(free_gpr, existing_gpr)
                } else {
                    panic!("This case cannot happen")
                }
            }
            RegAllocIntention::OutSlice(slice) => {
                let Some(existing_gpr) = existing_gpr else {
                    // The out slice is not loaded yet, we need to load it now.
                    self.gp[free_gpr].ty = RegAllocType::OccupiedDirty(slice);
                    return Ok(RegisterLoadInfo::Unloaded(free_gpr));
                };
                // Promote old gpr to occupied dirty such that it is flushed correctly.
                self.gp[existing_gpr].ty = RegAllocType::OccupiedDirty(slice);
                if matches!(hint, RegAllocHint::Reallocate(_)) {
                    // The out slice is already loaded. Move the slice if necessary
                    self.realloc_gp(free_gpr, existing_gpr)
                } else {
                    // If we dont want to reallocate we just signal that the slice is still loaded
                    // after we have marked it dirty.
                    Ok(RegisterLoadInfo::Loaded(existing_gpr))
                }
            }
            RegAllocIntention::Realloc(slice) => {
                let Some(old_gpr) = existing_gpr else {
                    Err(JitError::SliceNotLoaded)?
                };
                self.realloc_gp(free_gpr, old_gpr)
            }
        }
    }

    fn realloc_gp(
        &mut self,
        free_gpr: GPRegister,
        old_gpr: GPRegister,
    ) -> Result<RegisterLoadInfo<GPRegister>, JitError> {
        if free_gpr == old_gpr {
            // No move necessary, if new = old
            return Ok(RegisterLoadInfo::Loaded(free_gpr));
        }
        // Set the same allocation as previously. If it was dirty we want to keep it that way.
        self.gp[free_gpr] = self.gp[old_gpr];
        self.gp[old_gpr].ty = RegAllocType::Free;
        Ok(RegisterLoadInfo::NeedRelocation {
            from: old_gpr,
            to: free_gpr,
        })
    }

    /// Finds a register to spill. The result is a tuple where the first element is the register
    /// to spill, the second element is the slice that is currently loaded, and the third element
    /// is a boolean that indicates whether this slot is dirty.
    fn find_register_to_spill(&mut self) -> (GPRegister, JIRSlice, bool) {
        // TODO: Use more sophisticated spilling heuristic. For example, LU + FIFO
        trace_jit!("Searching for register to spill in allocations: {}", self.gp);
        let mut iter = self.gp.iter_mut();
        let mut write_occupied = None;
        let to_flush = loop {
            match iter.next() {
                None => break write_occupied,
                Some((gpr, alloc)) => match &alloc.ty {
                    RegAllocType::Occupied(slice) if alloc.version < self.version => {
                        trace_jit!("Found a non-dirty register to spill");
                        break Some((gpr, *slice, false));
                    }
                    RegAllocType::OccupiedDirty(slice) if alloc.version < self.version => {
                        write_occupied.get_or_insert((gpr, *slice, true));
                    }
                    _ => {}
                },
            }
        }
        .expect("At least one register must be occupied");
        to_flush
        // todo!()
    }

    pub fn spill_one_register<AssemblerT, EmitterT>(
        &mut self,
        assembler: &mut AssemblerT,
        emit_flush: EmitterT,
    ) -> JITResult<GPRegister>
    where
        EmitterT: FlushEmitter<AssemblerT>,
    {
        let to_spill = self.find_register_to_spill();
        trace_jit!("Spilling register {:?} to slice {:?}.", to_spill.0, to_spill.1);
        if to_spill.2 {
            emit_flush(assembler, to_spill.0, to_spill.1)?;
        }
        self.gp[to_spill.0].ty = RegAllocType::Free;
        Ok(to_spill.0)
    }
}

pub enum RegisterLoadInfo<T> {
    Unloaded(T),
    Loaded(T),
    NeedRelocation { from: T, to: T },
}
