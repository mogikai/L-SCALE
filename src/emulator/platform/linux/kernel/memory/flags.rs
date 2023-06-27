use bitflags::bitflags;

bitflags! {
#[repr(C)]
pub struct MMAP_FLAGS : u64 {
        const MAP_SHARED            = 0x1;      // Share changes.
        const MAP_PRIVATE           = 0x2;      // Changes are private.
        const MAP_SHARED_VALIDATE   = 0x3;      // Share changes and validate extension flags.
        const MAP_FIXED             = 0x10;     // Interpret addr exactly.
        const MAP_ANONYMOUS         = 0x20;
        const MAP_GROWSDOWN         = 0x00100;  // Stack-like segment.
        const MAP_DENYWRITE         = 0x00800;  // ETXTBSY.
        const MAP_EXECUTABLE        = 0x01000;  // Mark it as an executable.
        const MAP_LOCKED            = 0x02000;  // Lock the mapping.
        const MAP_NORESERVE         = 0x04000;  // Don't check for reservations.
        const MAP_POPULATE          = 0x08000;  // Populate (prefault) pagetables.
        const MAP_NONBLOCK          = 0x10000;  // Do not block on IO.
        const MAP_STACK             = 0x20000;  // Allocation is for a stack.
        const MAP_HUGETLB           = 0x40000;  // Create huge page mapping.
        const MAP_SYNC              = 0x80000;  // Perform synchronous page faults for the mapping.
    }
}