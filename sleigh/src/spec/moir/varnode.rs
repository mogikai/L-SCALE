use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::new_typed_index_table_id;
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::field::FieldId;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
    MOIRScope,
    MOIRTemporaryId,
};
use crate::spec::moir::table::TableId;
use num::Saturating;
use std::fmt::{
    Display,
    Formatter,
};

#[derive(Debug)]
/// A varnode is the unit of data manipulated by p-code.
/// It is simply a contiguous sequence of bytes in some address space.
///
/// Considering all the provided values, the storage location corresponding to a specific
/// varnode can be interpreted as: `space_addr + offset..space_addr+offset+size`
pub struct MOIRVarNode {
    // TODO: SpaceId
    /// The space can be interpreted as an address to the start of a memory region that
    /// is designated for storing certain values. For example, there may be a register
    /// space that is designated to saving the cpus registers when they are evicted from
    /// the host register.
    pub space: SpaceId,
    /// The offset specifies the offset from the start address of the space.
    pub offset: u64,
    /// The size specifies the size in bytes of the varnode.
    pub size: MOIRSize,
}

/// A slice of an existing varnode. The slice is at most as large as the original varnode.
#[derive(Debug, Copy, Clone)]
pub struct MOIRSlice {
    /// The slice refers to a symbol. This can be for example a varnode, a subtable export, or a field.
    pub reference: MOIRReference,
    /// The start byte of the slice
    pub start: MOIRSize,
    /// The end byte of the slice.
    pub end: MOIRMaybeSize,
}

impl MOIRSlice {
    pub fn varnode(
        varnode: VarNodeId,
        start: MOIRSize,
        end: MOIRSize,
    ) -> Self {
        Self {
            reference: MOIRReference::VarNode(varnode),
            start,
            end: MOIRMaybeSize::Determined(end),
        }
    }

    pub fn temporary(
        temporary: MOIRTemporaryId,
        start: MOIRSize,
        end: MOIRMaybeSize,
    ) -> Self {
        Self {
            reference: MOIRReference::Temporary(temporary),
            start,
            end,
        }
    }

    pub fn inst_start() -> Self {
        Self {
            reference: MOIRReference::InstStart,
            start: 0,
            end: MOIRMaybeSize::Undetermined,
        }
    }

    pub fn inst_next() -> Self {
        Self {
            reference: MOIRReference::InstNext,
            start: 0,
            end: MOIRMaybeSize::Undetermined,
        }
    }

    pub fn inst_next2() -> Self {
        Self {
            reference: MOIRReference::InstNext2,
            start: 0,
            end: MOIRMaybeSize::Undetermined,
        }
    }

    pub fn size(&self) -> MOIRMaybeSize {
        match self.end {
            MOIRMaybeSize::Determined(end) => MOIRMaybeSize::Determined(end.saturating_sub(self.start)),
            v => v,
        }
    }

    pub fn slice(
        self,
        start: MOIRSize,
        end: MOIRSize,
    ) -> SleighCompilerResult<Self> {
        let MOIRMaybeSize::Determined(current_end) = self.end else {
            Err(SleighCompilerError::CannotSliceUndetermined)?
        };
        if end > current_end || start < self.start {
            Err(SleighCompilerError::InvalidSliceRange {
                current_start: self.start,
                current_end,
                new_start: start,
                new_end: end,
            })
        } else {
            Ok(MOIRSlice {
                reference: self.reference,
                start,
                end: MOIRMaybeSize::Determined(end),
            })
        }
    }

    /// Truncates the slice to the specified size.
    /// The size cannot be larger than the original slice.
    pub fn truncate(
        self,
        size: MOIRSize,
    ) -> SleighCompilerResult<Self> {
        self.slice(self.start, self.start + size)
    }

    /// Sets the slice to the specified size. If the size is already determined this results in an error
    /// if the sizes do not match.
    pub fn set_size(
        &mut self,
        size: MOIRSize,
    ) -> SleighCompilerResult {
        // TODO: Maybe we need to look up the size of the value this slice references?
        let new_end = self.start + size;
        match self.end {
            MOIRMaybeSize::Determined(end) => {
                if end != new_end {
                    Err(SleighCompilerError::InvalidSliceSize {
                        new: size,
                        current: end.saturating_sub(self.start),
                    })
                } else {
                    Ok(())
                }
            }
            MOIRMaybeSize::Undetermined => {
                self.end = new_end.into();
                Ok(())
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MOIRReference {
    Field(FieldId),
    SubTable(TableId),
    VarNode(VarNodeId),
    Temporary(MOIRTemporaryId),
    /// The InstX References are special references that are resolved
    /// to a constant as soon as the constructor is called.
    InstStart,
    InstNext,
    InstNext2,
}

// /// A value references either a constant, or a symbol that is later resolved to a const or a varnode.
// #[derive(Debug, Copy, Clone)]
// pub enum MOIRValue {
//     /// The value references a slice of a symbol
//     ValueSlice(MOIRSlice),
//     /// The value is a constant value that can be used directly
//     Const(MOIRConst),
// }

/// A local either directly contains a constant value, or refers to a local stored in the scope.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MOIRLocal {
    Local(MOIRLocalId),
    Const(MOIRConst),
}

impl Display for MOIRLocal {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRLocal::Local(local) => write!(f, "{}", local),
            MOIRLocal::Const(value) => write!(f, "{}", value),
        }
    }
}

impl From<MOIRLocalId> for MOIRLocal {
    fn from(local: MOIRLocalId) -> Self {
        MOIRLocal::Local(local)
    }
}

impl From<MOIRConst> for MOIRLocal {
    fn from(value: MOIRConst) -> Self {
        MOIRLocal::Const(value)
    }
}

macro_rules! impl_local_from_const {
    ($typ:ty, $expr:expr) => {
        impl From<$typ> for MOIRLocal {
            fn from(value: $typ) -> Self {
                $expr(value).into()
            }
        }
        impl From<$typ> for MOIRConst {
            fn from(value: $typ) -> Self {
                $expr(value)
            }
        }
    };
}

impl_local_from_const!(u128, MOIRConst::Const128);
impl_local_from_const!(u64, MOIRConst::Const64);
impl_local_from_const!(u32, MOIRConst::Const32);
impl_local_from_const!(u16, MOIRConst::Const16);
impl_local_from_const!(u8, MOIRConst::Const8);

impl MOIRLocal {
    pub fn slice(
        self,
        scope: &mut MOIRScope,
        start: MOIRSize,
        end: MOIRSize,
    ) -> SleighCompilerResult<Self> {
        let new_size = end.saturating_sub(start);
        let current_size = self
            .size(scope)
            .ok_or(SleighCompilerError::CannotSliceUndetermined)?;
        if new_size == 0 || new_size > current_size || start > end {
            Err(SleighCompilerError::InvalidSliceSize {
                new: new_size,
                current: current_size,
            })?
        }
        Ok(match self {
            MOIRLocal::Local(local) => MOIRLocal::Local(scope.slice_local(local, start, end)?),
            MOIRLocal::Const(value) => MOIRLocal::Const(value.slice(start, end)?),
        })
    }

    pub fn truncate(
        self,
        scope: &mut MOIRScope,
        size: MOIRSize,
    ) -> SleighCompilerResult<Self> {
        Ok(match self {
            MOIRLocal::Local(local) => MOIRLocal::Local(scope.truncate_local(local, size)?),
            MOIRLocal::Const(value) => MOIRLocal::Const(value.truncate(size)?),
        })
    }

    pub fn size(
        &self,
        scope: &MOIRScope,
    ) -> MOIRMaybeSize {
        match self {
            MOIRLocal::Local(local) => scope.size_of_local(*local),
            MOIRLocal::Const(value) => value.size(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MOIRConst {
    Undetermined(u128),
    Const128(u128),
    Const64(u64),
    Const32(u32),
    Const16(u16),
    Const8(u8),
    /// A generic const value with the specified size
    ConstN(u128, MOIRSize),
}

impl Display for MOIRConst {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRConst::Undetermined(value) => write!(f, "{:#0x}", value),
            MOIRConst::Const128(value) => write!(f, "{:#0x}", value),
            MOIRConst::Const64(value) => write!(f, "{:#0x}", value),
            MOIRConst::Const32(value) => write!(f, "{:#0x}", value),
            MOIRConst::Const16(value) => write!(f, "{:#0x}", value),
            MOIRConst::Const8(value) => write!(f, "{:#0x}", value),
            MOIRConst::ConstN(value, _) => write!(f, "{:#0x}", value),
        }
    }
}

impl MOIRConst {
    /// Create a new const value with the specified size in bytes. Allowed sizes currently are 8,4,2,1.
    /// If the value is too large to fit in the specified size, an error is returned.
    /// If size is undetermined, we simply save the unconstrained const value such that it can be
    /// constrained later on.
    pub fn new_with_size(
        val: u128,
        size: MOIRMaybeSize,
    ) -> SleighCompilerResult<Self> {
        use MOIRMaybeSize::*;
        match size {
            Undetermined => Ok(MOIRConst::Undetermined(val)),
            Determined(16) => Ok(MOIRConst::Const128(val)),
            Determined(8) => Ok(MOIRConst::Const64(
                val.try_into()
                    .map_err(|_| SleighCompilerError::ConstDoesNotFit { val, size: 8 })?,
            )),
            Determined(4) => Ok(MOIRConst::Const32(
                val.try_into()
                    .map_err(|_| SleighCompilerError::ConstDoesNotFit { val, size: 4 })?,
            )),
            Determined(2) => Ok(MOIRConst::Const16(
                val.try_into()
                    .map_err(|_| SleighCompilerError::ConstDoesNotFit { val, size: 2 })?,
            )),
            Determined(1) => Ok(MOIRConst::Const8(
                val.try_into()
                    .map_err(|_| SleighCompilerError::ConstDoesNotFit { val, size: 1 })?,
            )),
            Determined(size) => Ok(MOIRConst::ConstN(val, size)),
        }
    }

    pub fn size(&self) -> MOIRMaybeSize {
        match self {
            MOIRConst::Const128(_) => MOIRMaybeSize::Determined(16),
            MOIRConst::Const64(_) => MOIRMaybeSize::Determined(8),
            MOIRConst::Const32(_) => MOIRMaybeSize::Determined(4),
            MOIRConst::Const16(_) => MOIRMaybeSize::Determined(2),
            MOIRConst::Const8(_) => MOIRMaybeSize::Determined(1),
            MOIRConst::Undetermined(_) => MOIRMaybeSize::Undetermined,
            MOIRConst::ConstN(_, size) => MOIRMaybeSize::Determined(*size),
        }
    }

    pub fn truncate(
        self,
        size: MOIRSize,
    ) -> SleighCompilerResult<MOIRConst> {
        self.slice(0, size)
    }

    /// Assign a size to the const if it does not already have one.
    /// If the const already has a size that differs from the new one,
    /// an error is returned.
    pub fn constrain_size(
        &mut self,
        size: MOIRSize,
    ) -> SleighCompilerResult {
        match self {
            MOIRConst::Undetermined(v) => {
                *self = MOIRConst::Const128(*v);
            }
            v => {
                if v.size() != MOIRMaybeSize::Determined(size) {
                    Err(SleighCompilerError::InvalidSliceSize {
                        new: size,
                        current: v.size().determined_or(0),
                    })?;
                }
            }
        };
        Ok(())
    }

    fn slice(
        self,
        start: MOIRSize,
        end: MOIRSize,
    ) -> SleighCompilerResult<MOIRConst> {
        let new_size = end.saturating_sub(start);
        // For each byte we need to shift 8 bit.
        let start_bit = start * 8;
        // We can only slice if the underlying value has a size already.
        let MOIRMaybeSize::Determined(size) = self.size() else {
            Err(SleighCompilerError::CannotSliceUndetermined)?
        };
        if new_size > size {
            Err(SleighCompilerError::InvalidSliceSize {
                new: new_size,
                current: size,
            })?
        }
        Ok(match self {
            MOIRConst::Const128(v) => {
                let v: u128 = v >> start_bit;
                match new_size {
                    16 => MOIRConst::Const128(v),
                    8 => MOIRConst::Const64(v as _),
                    4 => MOIRConst::Const32(v as _),
                    2 => MOIRConst::Const16(v as _),
                    1 => MOIRConst::Const8(v as _),
                    _ => {
                        todo!("Generate error. We do not support const that are not u8, u16, u32, or u64")
                    }
                }
            }
            MOIRConst::Const64(v) => {
                let v: u64 = v >> start_bit;
                match new_size {
                    8 => MOIRConst::Const64(v),
                    4 => MOIRConst::Const32(v as _),
                    2 => MOIRConst::Const16(v as _),
                    1 => MOIRConst::Const8(v as _),
                    _ => {
                        todo!("Generate error. We do not support const that are not u8, u16, u32, or u64")
                    }
                }
            }
            MOIRConst::Const32(v) => {
                let v: u32 = v >> start_bit;
                match new_size {
                    4 => MOIRConst::Const32(v as _),
                    2 => MOIRConst::Const16(v as _),
                    1 => MOIRConst::Const8(v as _),
                    _ => {
                        todo!("Generate error. We do not support const that are not u8, u16, u32, or u64")
                    }
                }
            }
            MOIRConst::Const16(v) => {
                let v: u16 = v >> start_bit;
                match new_size {
                    2 => MOIRConst::Const16(v as _),
                    1 => MOIRConst::Const8(v as _),
                    _ => {
                        todo!("Generate error. We do not support const that are not u8, u16, u32, or u64")
                    }
                }
            }
            MOIRConst::Const8(v) => {
                let v: u8 = v >> start_bit;
                match new_size {
                    1 => MOIRConst::Const8(v as _),
                    _ => {
                        todo!("Generate error. We do not support const that are not u8, u16, u32, or u64")
                    }
                }
            }
            MOIRConst::ConstN(v, size) => {
                todo!("ConstN slicing currently unsupported")
            }
            MOIRConst::Undetermined(_) => Err(SleighCompilerError::CannotSliceUndetermined)?,
        })
    }
}

new_typed_index_table_id! { pub struct VarNodeId; }
