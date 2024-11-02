use crate::compiler::error::{ConstDoesNotFit, SleighCompilerError, SliceError};
use crate::compiler::SleighCompilerResult;
use crate::compiler::error::{
    ConstDoesNotFit,
    SliceError,
    SleighCompilerError
};
use crate::new_typed_index_table_id;
use crate::spec::moir::MOIR_ADDRESS_SIZE;
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

    pub fn refers_to_varnode(&self) -> bool {
        match self.reference {
            MOIRReference::VarNode(_) | MOIRReference::Temporary(_) => true,
            _ => false,
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
            end: MOIRMaybeSize::Determined(MOIR_ADDRESS_SIZE),
        }
    }

    pub fn inst_next() -> Self {
        Self {
            reference: MOIRReference::InstNext,
            start: 0,
            end: MOIRMaybeSize::Determined(MOIR_ADDRESS_SIZE),
        }
    }

    pub fn inst_next2() -> Self {
        Self {
            reference: MOIRReference::InstNext2,
            start: 0,
            end: MOIRMaybeSize::Determined(MOIR_ADDRESS_SIZE),
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
            Err(SleighCompilerError::from_error(SliceError::UndeterminedSize.into()))?
        };
        if end > current_end || start < self.start {
            Err(SleighCompilerError::from_error(
                SliceError::InvalidSliceRange {
                    current_start: self.start,
                    current_end,
                    new_start: start,
                    new_end: end,
                }.into()
            ))
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
                    Err(SleighCompilerError::from_error(
                        (SliceError::InvalidSliceSize {
                            new: size,
                            current: end.saturating_sub(self.start),
                        }).into()))
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

impl Display for MOIRSlice {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}<{},{}>", self.reference, self.start, self.end)
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

impl Display for MOIRReference {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRReference::Field(id) => write!(f, "{id}"),
            MOIRReference::SubTable(id) => write!(f, "{id}"),
            MOIRReference::VarNode(id) => write!(f, "{id}"),
            MOIRReference::Temporary(id) => write!(f, "{id}"),
            MOIRReference::InstStart => write!(f, "inst_start"),
            MOIRReference::InstNext => write!(f, "inst_next"),
            MOIRReference::InstNext2 => write!(f, "inst_next2"),
        }
    }
}

/// A local either directly contains a constant value, or refers to a local stored in the scope.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MOIRLocal {
    Local(MOIRLocalId),
    Const(MOIRConst),
}

impl PartialEq<MOIRLocalId> for MOIRLocal {
    fn eq(
        &self,
        other: &MOIRLocalId,
    ) -> bool {
        self.eq(&MOIRLocal::from(*other))
    }
}

impl PartialEq<MOIRLocal> for MOIRLocalId {
    fn eq(
        &self,
        other: &MOIRLocal,
    ) -> bool {
        MOIRLocal::from(*self).eq(other)
    }
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
    ($typ:ty) => {
        impl From<$typ> for MOIRLocal {
            fn from(value: $typ) -> Self {
                MOIRConst::Determined(u128::from(value), size_of::<$typ>() as MOIRSize).into()
            }
        }
        impl From<$typ> for MOIRConst {
            fn from(value: $typ) -> Self {
                MOIRConst::Determined(u128::from(value), size_of::<$typ>() as MOIRSize)
            }
        }
    };
}

impl_local_from_const!(u128);
impl_local_from_const!(u64);
impl_local_from_const!(u32);
impl_local_from_const!(u16);
impl_local_from_const!(u8);

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
            .ok_or(SleighCompilerError::from_error(SliceError::UndeterminedSize.into()))?;
        if new_size == 0 || new_size > current_size || start > end {
            Err(SleighCompilerError::from_error(SliceError::InvalidSliceSize {
                new: new_size,
                current: current_size,
            }.into()))?
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
    /// A generic const value with the specified size
    Determined(u128, MOIRSize),
}

impl Display for MOIRConst {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            MOIRConst::Undetermined(value) => write!(f, "{:#0x}", value),
            MOIRConst::Determined(value, _) => write!(f, "{:#0x}", value),
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
        size: impl Into<MOIRMaybeSize>,
    ) -> SleighCompilerResult<Self> {
        use MOIRMaybeSize::*;
        let size = size.into();
        match size {
            Undetermined => Ok(MOIRConst::Undetermined(val)),
            Determined(size) => Ok(MOIRConst::Determined(val, size)),
        }
    }

    pub fn size(&self) -> MOIRMaybeSize {
        match self {
            MOIRConst::Undetermined(_) => MOIRMaybeSize::Undetermined,
            MOIRConst::Determined(_, size) => MOIRMaybeSize::Determined(*size),
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
                *self = MOIRConst::new_with_size(*v, size)?;
            }
            v => {
                if v.size() != MOIRMaybeSize::Determined(size) {
                    Err(SleighCompilerError::from_error(
                        SliceError::InvalidSliceSize {
                            new: size,
                            current: v.size().determined_or(0),
                        }.into()
                    ))?;
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
            Err(SleighCompilerError::from_error(SliceError::UndeterminedSize.into()))?
        };
        if new_size > size {
            let e = SliceError::InvalidSliceSize {
                new: new_size,
                current: size,
            };
            Err(SleighCompilerError::from_error(e.into()))?
        }
        Ok(match self {
            MOIRConst::Determined(value, size) => {
                let value = value >> start_bit;
                MOIRConst::Determined(value, new_size)
            }
            MOIRConst::Undetermined(_) => Err(SleighCompilerError::from_error(SliceError::UndeterminedSize.into()))?,
        })
    }
}

new_typed_index_table_id! { pub struct VarNodeId; "V"; }
