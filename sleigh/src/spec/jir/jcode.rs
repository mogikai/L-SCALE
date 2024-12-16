pub mod display;
mod inline_code;

use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::custom_op::CustomOpId;
pub use inline_code::*;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct JIRInstruction {
    pub output: Option<JIRSlice>,
    pub input_a: Option<JIRValue>,
    pub input_b: Option<JIRValue>,
    pub operation: JIROperation,
}

impl JIRInstruction {
    pub fn new_binary_op<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
        operation: JIROperation,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        let output = output.into();
        let input_a = input_a.into();
        let input_b = input_b.into();
        Self {
            output: Some(output),
            input_a: Some(input_a),
            input_b: Some(input_b),
            operation,
        }
    }

    pub fn new_unary_op<O, I>(
        output: O,
        input_a: I,
        operation: JIROperation,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        let output = output.into();
        let input_a = input_a.into();
        Self {
            output: Some(output),
            input_a: Some(input_a),
            input_b: None,
            operation,
        }
    }

    pub fn new_branch<I>(
        target: I,
        operation: JIROperation,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        Self {
            output: None,
            input_a: Some(target.into()),
            input_b: None,
            operation,
        }
    }

    pub fn new_conditional_branch<I>(
        target: I,
        cond: I,
        operation: JIROperation,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        Self {
            output: None,
            input_a: Some(target.into()),
            input_b: Some(cond.into()),
            operation,
        }
    }

    pub fn new_jcode_branch<I>(
        cond: Option<I>,
        operation: JIROperation,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        Self {
            output: None,
            input_a: None,
            input_b: cond.map(I::into),
            operation,
        }
    }

    pub fn new_custom_call(
        output: Option<impl Into<JIRSlice>>,
        op_id: CustomOpId,
        args: Vec<JIRValue>,
    ) -> Self {
        Self {
            output: output.map(Into::into),
            input_a: args.get(0).copied(),
            input_b: args.get(1).copied(),
            operation: JIROperation::CustomOp {
                op_id,
                extra_args: Rc::new(args.into_iter().skip(2).collect()),
            },
        }
    }

    /// Loads the value pointed to by the address at input_a to the varnode output.
    pub fn load(
        output: impl Into<JIRSlice>,
        addr: impl Into<JIRValue>,
        space: SpaceId,
    ) -> Self {
        Self {
            output: Some(output.into()),
            input_a: Some(addr.into()),
            input_b: None,
            operation: JIROperation::Load(space),
        }
    }

    /// Storing a value does not have any output varnode, since the memory referenced by addr is changed.
    /// The address is provided in input_a whereas the value is provided in input_b
    pub fn store(
        addr: impl Into<JIRValue>,
        space: SpaceId,
        value: impl Into<JIRValue>,
    ) -> Self {
        Self {
            output: None,
            input_a: Some(addr.into()),
            input_b: Some(value.into()),
            operation: JIROperation::Store(space),
        }
    }

    pub fn is_unconditional_branch(&self) -> bool {
        use JIROperation::*;
        matches!(
            self.operation,
            CallDirect | CallIndirect | ReturnDirect | ReturnIndirect | GotoDirect | GotoIndirect | JCodeGoto(_)
        )
    }

    pub fn ist_conditional_branch(&self) -> bool {
        use JIROperation::*;
        matches!(self.operation, GotoDirectIf | GotoIndirectIf | JCodeGotoIf(_))
    }
}

#[derive(Debug, Clone)]
pub enum JIROperation {
    // TODO: Remove this dummy op
    Unimpl,
    // -------------------------- GENERIC LOAD/STORE --------------------------
    Copy,
    /// output: The target slice to load into <br>
    /// input_a: The address to load from. <br>
    /// The size of the load is determined by the size of the output slice.
    Load(SpaceId),
    /// input_a: The address to store to <br>
    /// input_b: The value to store <br>
    /// The size of the store is determined by the size of the input_b slice.
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
    CustomOp {
        op_id: CustomOpId,
        extra_args: Rc<Vec<JIRValue>>,
    },
}
