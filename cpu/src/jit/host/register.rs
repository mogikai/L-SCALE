//! The definitions in this module are for the most part adapted from regalloc2
use std::fmt::{
    Debug,
    Display,
};
use strum::EnumCount;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, EnumCount)]
pub enum RegClass {
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
    pub const HW_ENC_MASK: u8 = (1 << Self::MAX_BITS) - 1;
    pub const NUM_INDEX: usize = 1 << (Self::MAX_BITS + 2); // including RegClass bits

    /// Create a new PReg. The `hw_enc` range is 6 bits.
    #[inline(always)]
    pub const fn new(
        hw_enc: u8,
        class: RegClass,
    ) -> Self {
        debug_assert!(hw_enc <= HostReg::HW_ENC_MASK);
        HostReg {
            bits: ((class as u8) << Self::MAX_BITS) | hw_enc,
        }
    }

    /// The physical register number, as encoded by the ISA for the particular register class.
    #[inline(always)]
    pub const fn hw_enc(self) -> u8 {
        self.bits & Self::HW_ENC_MASK
    }

    /// The register class.
    #[inline(always)]
    pub const fn class(self) -> RegClass {
        match (self.bits >> Self::MAX_BITS) & 0b11 {
            0 => RegClass::Int,
            1 => RegClass::Float,
            2 => RegClass::Vector,
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
            bits: index as u8,
        }
    }

    /// Return the "invalid PReg", which can be used to initialize
    /// data structures.
    #[inline(always)]
    pub const fn invalid() -> Self {
        HostReg::new(Self::HW_ENC_MASK, RegClass::Int)
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
            RegClass::Int => "i",
            RegClass::Float => "f",
            RegClass::Vector => "v",
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
        let mut mapping = Vec::<Option<T>>::default();
        mapping.resize_with(HostReg::NUM_INDEX, || None::<T>);
        Self { mapping }
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

    pub fn remove_set(
        &mut self,
        reg_set: HostRegSet,
    ) {
        for reg in reg_set {
            self.mapping[reg.index()] = None;
        }
    }

    /// Moves the contents of the slot at `src_reg` to `dst_reg`.
    /// Returns the item that was previously at `dst_reg` if there was one.
    pub fn r#move(
        &mut self,
        dst_reg: HostReg,
        src_reg: HostReg,
    ) -> Option<T> {
        let out = self.mapping[dst_reg.index()].take();
        self.mapping[dst_reg.index()] = self.mapping[src_reg.index()].take();
        out
    }

    /// Removes the associated item for the provided [HostReg],
    /// returning it, if there was a previous element.
    /// Otherwise [None] is returned.
    pub fn take(
        &mut self,
        reg: HostReg,
    ) -> Option<T> {
        self.mapping[reg.index()].take()
    }

    pub fn iter(&self) -> impl Iterator<Item = (HostReg, &T)> {
        self.mapping
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .as_ref()
                    .map(|value| (HostReg::from_index(index), value))
            })
    }

    /// Iterate over all registers from the specified class.
    pub fn iter_class(
        &self,
        reg_class: RegClass,
    ) -> impl Iterator<Item = (HostReg, &T)> {
        self.iter().filter(move |(reg, _)| reg.class() == reg_class)
    }

    /// Drains the entire host reg map, returning an iterator over all contained mappings.
    /// The register map will be empty afterward.
    pub fn drain(&mut self) -> impl Iterator<Item = (HostReg, T)> {
        self.mapping
            .iter_mut()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .take()
                    .map(|value| (HostReg::from_index(index), value))
            })
    }
}

impl<T> HostRegMap<T>
where
    T: Clone,
{
    pub fn clear(&mut self) {
        self.mapping.fill(None);
    }
}

/// A host register set. Used to represent clobbers efficiently.
///
/// The set is `Copy` and is guaranteed to have constant, and small,
/// size, as it is based on a bitset internally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct HostRegSet {
    // 64 bits, because register numbers can be up to 6 bits in length. 2^6 = 64
    bits: [u64; RegClass::COUNT],
}

impl HostRegSet {
    /// Create an empty set.
    pub const fn empty() -> Self {
        Self {
            bits: [0; RegClass::COUNT],
        }
    }

    /// Returns whether the given register is part of the set.
    pub fn contains(
        &self,
        reg: HostReg,
    ) -> bool {
        self.bits[reg.class() as usize] & (1 << reg.hw_enc()) != 0
    }

    /// Add a physical register (PReg) to the set, returning the new value.
    pub const fn with(
        self,
        reg: HostReg,
    ) -> Self {
        let mut out = self;
        out.bits[reg.class() as usize] |= 1 << reg.hw_enc();
        out
    }

    pub const fn without(
        self,
        reg: HostReg,
    ) -> Self {
        let mut out = self;
        out.bits[reg.class() as usize] &= !(1 << reg.hw_enc());
        out
    }

    /// Add a physical register (PReg) to the set.
    pub fn add(
        &mut self,
        reg: HostReg,
    ) {
        self.bits[reg.class() as usize] |= 1 << reg.hw_enc();
    }

    /// Remove a physical register (PReg) from the set.
    pub const fn remove(
        &mut self,
        reg: HostReg,
    ) {
        self.bits[reg.class() as usize] &= !(1 << reg.hw_enc());
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

    pub fn iter_class(
        &self,
        reg_class: RegClass,
    ) -> impl Iterator<Item = HostReg> + use<> {
        HostRegClassIter {
            bits: self.bits[reg_class as usize],
            reg_class,
        }
    }

    pub fn class_is_empty(
        &self,
        regclass: RegClass,
    ) -> bool {
        self.bits[regclass as usize] == 0
    }

    pub fn is_empty(&self) -> bool {
        for bits in self.bits {
            if bits != 0 {
                return false;
            }
        }
        true
    }

    pub fn get_nth_from_class(
        &self,
        n: usize,
        regclass: RegClass,
    ) -> Option<HostReg> {
        if n >= self.bits[regclass as usize].count_ones() as usize {
            return None;
        }
        let bits = self.bits[regclass as usize];
        let mut count = 0;
        for i in 0..64u64 {
            if (bits & (1 << i)) != 0 {
                if count == n {
                    return Some(HostReg::new(i as u8, regclass));
                }
                count += 1;
            }
        }
        None
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

pub struct HostRegClassIter {
    bits: u64,
    reg_class: RegClass,
}

impl Iterator for HostRegClassIter {
    type Item = HostReg;
    fn next(&mut self) -> Option<HostReg> {
        loop {
            if self.bits == 0 {
                return None;
            }
            let bit = self.bits.trailing_zeros();
            self.bits &= !(1 << bit);
            let hw_enc = bit as u8;
            return Some(HostReg::new(hw_enc, self.reg_class));
        }
    }
}

pub struct PRegSetIter {
    bits: [u64; RegClass::COUNT],
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
                let index = bit as usize | self.cur << HostReg::MAX_BITS;
                return Some(HostReg::from_index(index));
            }
            self.cur += 1;
        }
    }
}

impl Display for HostRegSet {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter,
    ) -> core::fmt::Result {
        write!(f, "HostRegSet[")?;
        let mut iter = self.into_iter().peekable();
        while let Some(reg) = iter.next() {
            write!(f, "{}", reg)?;
            if iter.peek().is_some() {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}
