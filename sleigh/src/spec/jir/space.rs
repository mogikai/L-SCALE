use crate::spec::jir::common::JIRSize;

#[derive(Debug, Clone)]
pub struct JIRSpace {
    pub r#type: JIRSpaceType,
    /// The number of bytes an address in the space takes up
    pub address_size: JIRSize,
    /// The size of data in bytes accessed by an address.
    pub wordsize: JIRSize,
}

#[derive(Debug, Copy, Clone)]
pub enum JIRSpaceType {
    /// A RAM space represents random access memory. This space is usually backed by some kind of
    /// memory implementation. Access to this space is handled by the JIT by generating memory loads
    /// and stores accordingly. The backing implementation may for example provide a simple one to one
    /// mapping of [EVAddr](mogi_utilities::addresses::EVAddr) to [EPAddr](mogi_utilities::addresses::EPAddr),
    /// or provide some kind of page-table implementation. An [EPAddr](mogi_utilities::addresses::EPAddr)
    /// is an emulated physical address and may map directly to a host virtual address
    /// ([HVAddr](mogi_utilities::addresses::HVAddr)), or may be mapped by a simple table.
    /// For more details refer to the mogi_mem crate.
    Ram,
    /// A ROM space is just a special case of a RAM space. The only difference is that it will be mapped read only.
    Rom,
    /// A register space contains all the registers of the target architecture. This space needs
    /// to be stored in the CPU context of an emulated cpu. The VarNodes in this space will be
    /// loaded or flushed by the JIT.
    Register,
    /// A const space is a special space where the offset of a varnode corresponds to its value.
    Const,
    /// A temporary space contains temporary varnodes that are needed by the JIR that is compiled from
    /// the PCode in the SLEIGH specification. These temporary varnodes are not visible outside
    /// an instruction. If the instruction needs to call some internal helper function, then these
    /// temporary varnodes may need to be accessed.
    Temporary,
}
