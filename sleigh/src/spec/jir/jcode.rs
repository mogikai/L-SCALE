mod inline_code;

use crate::new_typed_index_table_id;
use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::custom_op::CustomOpId;
pub use inline_code::*;
use slotmap::new_key_type;

#[derive(Debug)]
pub struct JIRInstruction {
    pub output: Option<JIRSlice>,
    pub input_a: Option<JIRValue>,
    pub input_b: Option<JIRValue>,
    pub operation: JIROperation,
}

impl JIRInstruction {
    pub fn new_binary_op(
        output: impl Into<JIRSlice>,
        input_a: impl Into<JIRValue>,
        input_b: impl Into<JIRValue>,
        operation: JIROperation,
    ) -> Self {
        Self {
            output: Some(output.into()),
            input_a: Some(input_a.into()),
            input_b: Some(input_b.into()),
            operation,
        }
    }

    pub fn new_unary_op(
        output: impl Into<JIRSlice>,
        input_a: impl Into<JIRValue>,
        operation: JIROperation,
    ) -> Self {
        Self {
            output: Some(output.into()),
            input_a: Some(input_a.into()),
            input_b: None,
            operation,
        }
    }

    pub fn is_unconditional_branch(&self) -> bool {
        use JIROperation::*;
        match self.operation {
            CallDirect | CallIndirect | ReturnDirect | ReturnIndirect | GotoDirect | GotoIndirect | JCodeGoto(_) => {
                true
            }
            _ => false,
        }
    }

    pub fn ist_conditional_branch(&self) -> bool {
        use JIROperation::*;
        match self.operation {
            GotoDirectIf | GotoIndirectIf | JCodeGotoIf(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum JIROperation {
    // -------------------------- GENERIC LOAD/STORE --------------------------
    Copy,
    Load(SpaceId),
    Store(SpaceId),
    // -------------------------- INTEGER ARITHMETIC --------------------------
    // Binary operations
    IntAdd,
    IntSubtract,
    IntMul,
    IntDiv,
    IntSDiv,
    IntRemainder,
    IntSRemainder,
    IntLeft,
    IntRight,
    IntSRight,
    IntOr,
    IntAnd,
    IntXor,
    // Comparisons
    IntSLess,
    IntSGreater,
    IntSLessEqual,
    IntSGreaterEqual,
    IntLess,
    IntGreater,
    IntLessEqual,
    IntGreaterEqual,
    IntEquals,
    IntNotEquals,
    // Unary operations
    BitNegate,
    IntNegate,
    // --------------------------- FLOAT ARITHMETIC ---------------------------
    // Binary operations
    FloatDiv,
    FloatMul,
    FloatAdd,
    FloatSubtract,
    // Comparisons
    FloatLess,
    FloatGreater,
    FloatLessEqual,
    FloatGreaterEqual,
    FloatEquals,
    FloatNotEquals,
    // Unary operations
    FloatNegate,

    // -------------------------- BOOLEAN OPERATIONS --------------------------
    // Binary operations
    BoolOr,
    BoolAnd,
    BoolXor,
    // Unary operations
    BoolNegate,
    // ------------------------- BRANCHING OPERATIONS -------------------------
    // Same as goto, but semantically different for analysis purposes.
    CallDirect,
    CallIndirect,

    // Same as goto, but semantically different for analysis purposes.
    ReturnDirect,
    ReturnIndirect,

    // Simple Goto
    GotoDirect,
    GotoIndirect,
    GotoDirectIf,
    GotoIndirectIf,
    // Internal branches. JCode internal branches can only jump to other blocks inside a single TargetInstruction
    JCodeGoto(JIRBlockId),
    JCodeGotoIf(JIRBlockId),

    // -------------------------- SPECIAL OPERATIONS --------------------------
    PopCount,
    LZCount,
    ZeroExtend,
    SignExtend,
    Carry,
    SignCarry,
    SignBorrow,
    Nan,
    Abs,
    Sqrt,
    IntToFloat,
    FloatToFloat,
    Trunc,
    Ceil,
    Floor,
    Round,
    CustomOp(CustomOpId),
}
