//! This crate provides an emulator and a fuzzer that can be used to fuzz user space binary applications.
//! The emulator currently only supports x86 binaries that are intended to run on a linux kernel.
//!
//! TODO: EXAMPLES HERE
//!
#![feature(try_blocks)]
#![feature(map_try_insert)]
#![feature(iterator_try_collect)]
#![allow(dead_code)]
#![allow(unused)]
#![allow(unused_variables)]
// #![warn(missing_docs)]

pub mod emulator;
pub mod fuzzer;

#[macro_use]
extern crate log;
extern crate core;
extern crate derive_more;

use static_assertions::assert_eq_size;

// This library emulates 64 bit targets and thus requires usize to be 64 bits
assert_eq_size!(u64, usize);
