//! # Mogi CPU
//! This crate implements a cpu emulator which can be used stand alone to emulate short assembly snippets,
//! or in conjunction with a fully fledged system emulator.
//!
//! The Emulation process can be divided into the following steps:
//! 1. Compile the SLEIGH processor specification using the [SleighCompiler](mogi_sleigh::compiler::SleighCompiler)
//! 2. Register helper functions for [custom operations](crate::jit::api::MogiJit::register_custom_op_impl)
//!    and [memory paging](crate::jit::api::MogiJit::register_tlb_read_miss_callback)
//! 3. For each instruction or block:
//!     1. Lift the instruction bytes into [JIR](mogi_sleigh::spec::jir)
//!     2. Compile the [JIR](mogi_sleigh::spec::jir) into host assembly with the corresponding [JIT compiler](crate::jit::api::MogiJit)
//!     3. Execute the jited block.
//!
//! Instead of re-translating blocks, the jit will usually cache already translated code and simply fetch the translated block.
//! TODO: Who is responsible for notifying the jit of memory changes on executable pages? The jit needs to clear the block
//!       cache for these addresses.
//!
//! ## Memory translation
//! TODO: Describe how the tlb works, and how the tlb miss callbacks work
//!
#![feature(let_chains)]
#![feature(abort_unwind)]
// TODO: Disallow dead code once prototyping is done
#![allow(dead_code)]
#![deny(unsafe_op_in_unsafe_fn)]
extern crate core;

pub mod cpu;
pub mod cpu_context;
pub mod jit;
