#![feature(let_chains)] // Todo enable missing docs once prototyping phase is completed.

// #[deny(missing_docs)]
#![deny(clippy::cognitive_complexity)]
#![deny(clippy::unwrap_used)]
#![allow(dead_code)]
#![allow(unused_braces)]
extern crate core;

pub mod compiler;
pub mod lifter;
pub mod disassembler;
pub mod spec;

pub use mogi_sleigh_parser::preprocessor::SleighPreprocessorFactory;
