#![feature(let_chains)]
// Todo enable missing docs once prototyping phase is completed.

// #[deny(missing_docs)]
// #![deny(clippy::unwrap_used)]
#![deny(clippy::cognitive_complexity)]
#![allow(dead_code)]
#![allow(unused_braces)]
extern crate core;

pub mod compiler;
pub mod disassembler;
pub mod lifter;
pub mod spec;

pub use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;
