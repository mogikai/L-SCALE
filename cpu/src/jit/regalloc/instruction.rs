use crate::jit::host::register::HostReg;
use mogi_sleigh::spec::jir::values::JIRSlice;

/// The allocator produces instructions that contain loads or moves that need to be emitted by the emitter
/// in order to make sure that registers are correctly loaded.
pub enum AllocatorInstruction {
    /// Load the specified host register with the specified Slice.
    Load(HostReg, JIRSlice),
    Spill(HostReg, JIRSlice),
    /// Exchange the two host registers.
    Exchange(HostReg, HostReg),
}
