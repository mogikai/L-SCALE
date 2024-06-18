//! The intermediate representation *MIR* used in mogi_cpu.
//! The target instruction set (e.g. *ARM*) is lifted into *MIR*.
//! *MIR* is then translated into host assembly instructions (e.g. *x86*).
//! This code is then assembled and cached for further execution.

pub enum MIROperation {
    IntAdd,

    Branch,
    Ret,
}
