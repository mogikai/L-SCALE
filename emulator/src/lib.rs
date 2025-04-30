//! This crate provides an emulator and a fuzzer that can be used to fuzz user space binary applications.
//! The emulator currently only supports x86 binaries that are intended to run on a linux kernel.
//!
//! TODO: EXAMPLES HERE
//!
#![feature(try_blocks)]
#![feature(map_try_insert)]
#![feature(iterator_try_collect)]
#![feature(min_specialization)]
#![feature(error_generic_member_access)]
#![feature(try_trait_v2)]
#![feature(ptr_as_ref_unchecked)]
#![allow(dead_code)]
#![allow(unused)]
#![allow(unused_variables)]
#![allow(clippy::four_forward_slashes)]
#![deny(missing_debug_implementations)]
#![deny(rust_2018_idioms)]
#![deny(rust_2021_compatibility)]
// #![warn(missing_docs)]

pub use byteorder::{
    BigEndian,
    ByteOrder,
    LittleEndian,
};
use static_assertions::assert_eq_size;

pub mod emulator;
pub mod util;

// This library emulates 64 bit targets and thus requires usize to be 64 bits
assert_eq_size!(u64, usize);
