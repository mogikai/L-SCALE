#[macro_export]
macro_rules! jcode {
    ($to:ident = $from:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Copy)
    };
    ($to:ident = *[$space:tt]$addr:tt) => {
        $crate::spec::jir::jcode::JIRInstruction::new_unary_op($to, $from, $crate::spec::jir::jcode::JIROperation::Copy)
    };
}

pub use jcode;
