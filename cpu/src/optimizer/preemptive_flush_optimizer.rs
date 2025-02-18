//! Insert preemptive flush operations to signal the jit compiler that a varnode may be flushed now,
//! since there are no further read or write accesses to this varnode.
//! This makes it easier to allocate registers without flushing varnodes that are still needed.
// TODO: Measure performance impact of flushing directly vs flushing when the varnode is needed.
