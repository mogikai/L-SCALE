pub mod api;
pub mod assembler;
pub mod cache;
pub mod code_buffer;
pub mod x86;

#[cfg(feature = "trace_jit")]
macro_rules! trace_jit {
    ($($arg:tt)*) => {
        tracing::trace!($($arg)*)
    };
}
#[cfg(not(feature = "trace_jit"))]
macro_rules! trace_jit {
    ($($arg:tt)*) => {};
}

pub(crate) use trace_jit;
