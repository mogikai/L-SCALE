use crate::new_typed_index_table_id;
use crate::spec::moir::scope::MOIRMaybeSize;

/// A custom operation defined in the sleigh specification
/// The operation will be called from jited code. Any implementation
/// can expect to always be given the same number of inputs, and they
/// are guaranteed to always have the same sizes.
#[derive(Debug)]
pub enum MOIRCustomOp {
    /// The custom operation has only been declared so far. The first invocation will define it.
    Declared,
    Defined {
        /// The input constraints of the custom operation. The compiler will check if all calls
        /// satisfy the same constraints, i.e. the number of parameters and their sizes are equal.
        input_constraints: Vec<MOIRMaybeSize>,
        /// The output constraint. It may be None, if there is no output. See above.
        output: Option<MOIRMaybeSize>,
    },
}

new_typed_index_table_id! { pub struct CustomOpId; "OP"; }
