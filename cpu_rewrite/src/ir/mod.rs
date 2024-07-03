//! The intermediate representation *MIR* used in mogi_cpu.
//! The target instruction set (e.g. *ARM*) is lifted into *MIR*.
//! *MIR* is then translated into host assembly instructions (e.g. *x86*).
//! This code is then assembled and cached for further execution.

/// MIR does not have push instructions. A push must be performed, by manually adjusting the stack pointer,
/// and then loading or storing to/from the destination.
pub enum MIROperation {
    IntAdd,

    Branch,
    Ret,

    Load,
    Store,
}
