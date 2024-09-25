use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::custom_op::CustomOpId;

#[derive(Debug)]
pub struct JIRInstruction {
    output: Option<JIRSlice>,
    input_a: Option<JIRValue>,
    input_b: Option<JIRValue>,
    operation: JIROperation,
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
    JCodeGoto(),
    JCodeGotoIf(),

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
