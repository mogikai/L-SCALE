use crate::spec::moir::scope::MOIRMaybeSize;
use mogi_sleigh_parser::StringId;
use mogi_utilities::{
    new_map_key,
    new_map_key_16,
};

/// A custom operation defined in the sleigh specification
/// The operation will be called from jited code. Any implementation
/// can expect to always be given the same number of inputs, and they
/// are guaranteed to always have the same sizes.
#[derive(Debug)]
pub enum MOIRCustomOp {
    /// The custom operation has only been declared so far. The first invocation will define it.
    Declared { name: StringId },
    Defined {
        name: StringId,
        /// The input constraints of the custom operation. The compiler will check if all calls
        /// satisfy the same constraints, i.e. the number of parameters and their sizes are equal.
        input_constraints: Vec<MOIRMaybeSize>,
        /// The output constraint. It may be None, if there is no output. See above.
        output: Option<MOIRMaybeSize>,
    },
}

new_map_key_16! { pub struct CustomOpId; "OP"; }
