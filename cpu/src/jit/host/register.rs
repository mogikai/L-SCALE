//! The definitions in this module are for the most part adapted from regalloc2
use std::fmt::Debug;

/// Register classes.
///
/// Every value has a "register class", which is like a type at the
/// register-allocator level. Every register must belong to only one
/// class; i.e., they are disjoint.
///
/// For tight bit-packing throughout our data structures, we support
/// only three classes, "int", "float" and "vector". Usually two will
/// be enough on modern machines, as they have one class of general-purpose
/// integer registers of machine width (e.g. 64 bits), and another
/// class of float/vector registers used both for FP and for vector
/// operations. Additionally for machines with totally separate vector
/// registers a third class is provided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostRegClass {
    Int    = 0,
    Float  = 1,
    Vector = 2,
}

/// A physical register. Contains a physical register number and a class.
///
/// The `hw_enc` field contains the physical register number and is in
/// a logically separate index space per class; in other words, Int
/// register 0 is different than Float register 0.
///
/// Because of bit-packed encodings throughout the implementation,
/// `hw_enc` must fit in 6 bits, i.e., at most 64 registers per class.
///
/// The value returned by `index()`, in contrast, is in a single index
/// space shared by all classes, in order to enable uniform reasoning
/// about physical registers. This is done by putting the class bit at
/// the MSB, or equivalently, declaring that indices 0..=63 are the 64
/// integer registers and indices 64..=127 are the 64 float registers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostReg {
    bits: u8,
}

impl HostReg {
    pub const MAX_BITS: usize = 6;
    pub const MAX_HW_ENC: u8 = (1 << Self::MAX_BITS) - 1;
    pub const NUM_INDEX: usize = 1 << (Self::MAX_BITS + 2); // including RegClass bits

    /// Create a new PReg. The `hw_enc` range is 6 bits.
    #[inline(always)]
    pub const fn new(
        hw_enc: u8,
        class: HostRegClass,
    ) -> Self {
        debug_assert!(hw_enc <= HostReg::MAX_HW_ENC);
        HostReg {
            bits: ((class as u8) << Self::MAX_BITS) | (hw_enc as u8),
        }
    }

    /// The physical register number, as encoded by the ISA for the particular register class.
    #[inline(always)]
    pub const fn hw_enc(self) -> u8 {
        self.bits & Self::MAX_HW_ENC
    }

    /// The register class.
    #[inline(always)]
    pub const fn class(self) -> HostRegClass {
        match (self.bits >> Self::MAX_BITS) & 0b11 {
            0 => HostRegClass::Int,
            1 => HostRegClass::Float,
            2 => HostRegClass::Vector,
            _ => unreachable!(),
        }
    }

    /// Get an index into the (not necessarily contiguous) index space of
    /// all physical registers. Allows one to maintain an array of data for
    /// all PRegs and index it efficiently.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.bits as usize
    }

    /// Construct a PReg from the value returned from `.index()`.
    #[inline(always)]
    pub const fn from_index(index: usize) -> Self {
        HostReg {
            bits: (index & (Self::NUM_INDEX - 1)) as u8,
        }
    }

    /// Return the "invalid PReg", which can be used to initialize
    /// data structures.
    #[inline(always)]
    pub const fn invalid() -> Self {
        HostReg::new(Self::MAX_HW_ENC, HostRegClass::Int)
    }
}

impl Default for HostReg {
    fn default() -> Self {
        Self::invalid()
    }
}

impl Debug for HostReg {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter,
    ) -> core::fmt::Result {
        write!(f, "PReg(hw = {}, class = {:?}, index = {})", self.hw_enc(), self.class(), self.index())
    }
}

impl core::fmt::Display for HostReg {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter,
    ) -> core::fmt::Result {
        let class = match self.class() {
            HostRegClass::Int => "i",
            HostRegClass::Float => "f",
            HostRegClass::Vector => "v",
        };
        write!(f, "p{}{}", self.hw_enc(), class)
    }
}

/// Maps a host register to a value of type T
#[derive(Clone, Debug)]
pub struct HostRegMap<T> {
    mapping: Vec<Option<T>>,
}

impl<T> Default for HostRegMap<T> {
    fn default() -> Self {
        Self {
            mapping: Default::default(),
        }
    }
}

impl<T> HostRegMap<T> {
    pub fn insert(
        &mut self,
        reg: HostReg,
        value: T,
    ) {
        self.mapping[reg.index()] = Some(value);
    }

    pub fn get(
        &self,
        reg: HostReg,
    ) -> Option<&T> {
        self.mapping.get(reg.index())?.as_ref()
    }

    /// Removes the associated item for the provided [HostReg]
    pub fn remove(
        &mut self,
        reg: HostReg,
    ) {
        self.mapping[reg.index()] = None;
    }

    pub fn take(
        &mut self,
        reg: HostReg,
    ) -> Option<T> {
        self.mapping[reg.index()].take()
    }
}

/// A type for internal bit arrays.
type Bits = u64;

/// A host register set. Used to represent clobbers efficiently.
///
/// The set is `Copy` and is guaranteed to have constant, and small,
/// size, as it is based on a bitset internally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HostRegSet {
    bits: [Bits; Self::LEN],
}

impl HostRegSet {
    /// The number of bits per element in the internal bit array.
    const BITS: usize = core::mem::size_of::<Bits>() * 8;

    /// Length of the internal bit array.
    const LEN: usize = (HostReg::NUM_INDEX + Self::BITS - 1) / Self::BITS;

    /// Create an empty set.
    pub const fn empty() -> Self {
        Self { bits: [0; Self::LEN] }
    }

    /// Splits the given register index into parts to access the internal bit array.
    const fn split_index(reg: HostReg) -> (usize, usize) {
        let index = reg.index();
        (index >> Self::BITS.ilog2(), index & (Self::BITS - 1))
    }

    /// Returns whether the given register is part of the set.
    pub fn contains(
        &self,
        reg: HostReg,
    ) -> bool {
        let (index, bit) = Self::split_index(reg);
        self.bits[index] & (1 << bit) != 0
    }

    /// Add a physical register (PReg) to the set, returning the new value.
    pub const fn with(
        self,
        reg: HostReg,
    ) -> Self {
        let (index, bit) = Self::split_index(reg);
        let mut out = self;
        out.bits[index] |= 1 << bit;
        out
    }

    /// Add a physical register (PReg) to the set.
    pub fn add(
        &mut self,
        reg: HostReg,
    ) {
        let (index, bit) = Self::split_index(reg);
        self.bits[index] |= 1 << bit;
    }

    /// Remove a physical register (PReg) from the set.
    pub fn remove(
        &mut self,
        reg: HostReg,
    ) {
        let (index, bit) = Self::split_index(reg);
        self.bits[index] &= !(1 << bit);
    }

    /// Add all of the registers in one set to this one, mutating in
    /// place.
    pub fn union_from(
        &mut self,
        other: HostRegSet,
    ) {
        for i in 0..self.bits.len() {
            self.bits[i] |= other.bits[i];
        }
    }

    pub fn intersect_from(
        &mut self,
        other: HostRegSet,
    ) {
        for i in 0..self.bits.len() {
            self.bits[i] &= other.bits[i];
        }
    }

    pub fn invert(&self) -> HostRegSet {
        let mut set = self.bits;
        for i in 0..self.bits.len() {
            set[i] = !self.bits[i];
        }
        HostRegSet { bits: set }
    }

    pub fn is_empty(
        &self,
        regclass: HostRegClass,
    ) -> bool {
        self.bits[regclass as usize] == 0
    }
}

impl core::ops::BitAnd<HostRegSet> for HostRegSet {
    type Output = HostRegSet;

    fn bitand(
        self,
        rhs: HostRegSet,
    ) -> Self::Output {
        let mut out = self;
        out.intersect_from(rhs);
        out
    }
}

impl core::ops::BitOr<HostRegSet> for HostRegSet {
    type Output = HostRegSet;

    fn bitor(
        self,
        rhs: HostRegSet,
    ) -> Self::Output {
        let mut out = self;
        out.union_from(rhs);
        out
    }
}

impl IntoIterator for HostRegSet {
    type Item = HostReg;
    type IntoIter = PRegSetIter;
    fn into_iter(self) -> PRegSetIter {
        PRegSetIter {
            bits: self.bits,
            cur: 0,
        }
    }
}

pub struct PRegSetIter {
    bits: [Bits; HostRegSet::LEN],
    cur: usize,
}

impl Iterator for PRegSetIter {
    type Item = HostReg;
    fn next(&mut self) -> Option<HostReg> {
        loop {
            let bits = self.bits.get_mut(self.cur)?;
            if *bits != 0 {
                let bit = bits.trailing_zeros();
                *bits &= !(1 << bit);
                let index = bit as usize + self.cur * HostRegSet::BITS;
                return Some(HostReg::from_index(index));
            }
            self.cur += 1;
        }
    }
}
