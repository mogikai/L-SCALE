pub mod display;
pub mod eval;
mod inline_code;

use crate::spec::jir::jir_block::JIRBlockId;
use crate::spec::jir::values::{
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::custom_op::CustomOpId;
pub use inline_code::*;
use mogi_utilities::addresses::EVAddr;
use std::rc::Rc;

/// This enum is used to specify the instructions that may follow the current instruction.
/// It may be a direct jump to a constant address, or a direct or indirect jump to a
/// value calculated at runtime.
/// This always ends the current block.
#[derive(Debug, Clone)]
pub enum NextBlockTarget {
    Direct(EVAddr),
    DynamicDirect(JIRValue),
    DynamicIndirect(JIRValue),
    Return(EVAddr),
}

#[derive(Debug)]
pub enum ContinueLifting {
    Continue(EVAddr),
    Stop,
}

impl JIRInstruction {
    pub fn instruction_start(addr: EVAddr) -> Self {
        Self::InstructionStart(addr)
    }

    pub fn instruction_end(addr: EVAddr) -> Self {
        JIRInstruction::InstructionEnd(addr)
    }

    pub fn block_start(block_id: JIRBlockId) -> Self {
        JIRInstruction::BlockStart(block_id)
    }

    pub fn block_end(block_id: JIRBlockId) -> Self {
        JIRInstruction::BlockEnd(block_id)
    }

    pub fn int_add<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntAdd(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_subtract<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSubtract(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_mul<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntMul(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_div<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntDiv(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sdiv<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSDiv(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_remainder<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntRemainder(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sremainder<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSRemainder(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_left<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntLeft(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_right<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntRight(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sright<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSRight(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_or<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntOr(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_and<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntAnd(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_xor<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntXor(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sless<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSLess(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sgreater<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSGreater(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sless_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSLessEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_sgreater_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntSGreaterEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_less<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntLess(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_greater<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntGreater(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_less_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntLessEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_greater_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntGreaterEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_equals<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntEquals(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn int_not_equals<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::IntNotEquals(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_div<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatDiv(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_mul<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatMul(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_add<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatAdd(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_subtract<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatSubtract(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_less<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatLess(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_greater<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatGreater(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_less_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatLessEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_greater_equal<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatGreaterEqual(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_equals<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatEquals(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn float_not_equals<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::FloatNotEquals(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn bool_or<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::BoolOr(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn bool_and<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::BoolAnd(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn bool_xor<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::BoolXor(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn carry<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::Carry(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn sign_carry<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::SignCarry(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }
    pub fn sign_borrow<O, I1, I2>(
        output: O,
        input_a: I1,
        input_b: I2,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I1: Into<JIRValue>,
        I2: Into<JIRValue>,
    {
        JIRInstruction::SignBorrow(BinaryArgs {
            output: output.into(),
            input_a: input_a.into(),
            input_b: input_b.into(),
        })
    }

    pub fn copy<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Copy(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn bit_negate<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::BitNegate(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn int_negate<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::IntNegate(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn float_negate<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::FloatNegate(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn bool_negate<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::BoolNegate(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn pop_count<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::PopCount(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn lzcount<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::LZCount(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn zero_extend<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::ZeroExtend(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn sign_extend<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::SignExtend(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn nan<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Nan(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn abs<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Abs(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn sqrt<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Sqrt(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn int_to_float<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::IntToFloat(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn float_to_float<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::FloatToFloat(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn trunc<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Trunc(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn ceil<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Ceil(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn floor<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Floor(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }
    pub fn round<O, I>(
        output: O,
        input: I,
    ) -> Self
    where
        O: Into<JIRSlice>,
        I: Into<JIRValue>,
    {
        JIRInstruction::Round(UnaryArgs {
            output: output.into(),
            input: input.into(),
        })
    }

    pub fn goto_direct<I>(target: I) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::GotoDirect(target.into())
    }

    pub fn goto_indirect<I>(target: I) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::GotoIndirect(target.into())
    }

    pub fn call_direct<I>(
        target: I,
        retaddr: EVAddr,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::CallDirect(target.into(), retaddr)
    }

    pub fn call_indirect<I>(
        target: I,
        retaddr: EVAddr,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::CallIndirect(target.into(), retaddr)
    }

    pub fn return_direct<I>(target: I) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::ReturnDirect(target.into())
    }

    pub fn return_indirect<I>(target: I) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::ReturnIndirect(target.into())
    }

    pub fn goto_direct_if<T, C>(
        target: T,
        cond: C,
    ) -> Self
    where
        T: Into<JIRValue>,
        C: Into<JIRValue>,
    {
        JIRInstruction::GotoDirectIf {
            condition: cond.into(),
            target: target.into(),
        }
    }

    pub fn goto_indirect_if<T, C>(
        target: T,
        cond: C,
    ) -> Self
    where
        T: Into<JIRValue>,
        C: Into<JIRValue>,
    {
        JIRInstruction::GotoIndirectIf {
            condition: cond.into(),
            target: target.into(),
        }
    }

    pub fn jcode_goto_if<I>(
        cond: I,
        target_block: JIRBlockId,
    ) -> Self
    where
        I: Into<JIRValue>,
    {
        JIRInstruction::JCodeGotoIf {
            condition: cond.into(),
            target_block,
        }
    }

    pub fn jcode_goto(target_block: JIRBlockId) -> Self {
        JIRInstruction::JCodeGoto(target_block)
    }

    pub fn new_custom_call(
        output: Option<impl Into<JIRSlice>>,
        op_id: CustomOpId,
        args: Vec<JIRValue>,
    ) -> Self {
        JIRInstruction::CustomOp {
            output: output.map(Into::into),
            input_a: args.first().copied(),
            input_b: args.get(1).copied(),
            op_id,
            extra_args: Rc::new(args.into_iter().skip(2).collect()),
        }
    }

    /// Loads the value pointed to by the address at input_a to the varnode `target`.
    pub fn load(
        target: impl Into<JIRSlice>,
        address: impl Into<JIRValue>,
        space: SpaceId,
    ) -> Self {
        JIRInstruction::Load {
            target: target.into(),
            address: address.into(),
            space,
        }
    }

    /// Storing a value does not have any output varnode, since the memory referenced by `address` is changed.
    /// The address is provided in input_a whereas the value is provided in input_b
    pub fn store(
        address: impl Into<JIRValue>,
        space: SpaceId,
        value: impl Into<JIRValue>,
    ) -> Self {
        JIRInstruction::Store {
            address: address.into(),
            value: value.into(),
            space,
        }
    }

    pub fn nop() -> Self {
        JIRInstruction::Nop
    }

    pub fn is_branch(&self) -> bool {
        use JIRInstruction::*;
        matches!(
            self,
            CallDirect(..)
                | CallIndirect(..)
                | ReturnDirect(_)
                | ReturnIndirect(_)
                | GotoDirect(_)
                | GotoIndirect(_)
                | JCodeGoto(_)
                | GotoDirectIf { .. }
                | GotoIndirectIf { .. }
                | JCodeGotoIf { .. }
        )
    }

    pub fn is_unconditional_branch(&self) -> bool {
        use JIRInstruction::*;
        matches!(
            self,
            CallDirect(..)
                | CallIndirect(..)
                | ReturnDirect(_)
                | ReturnIndirect(_)
                | GotoDirect(_)
                | GotoIndirect(_)
                | JCodeGoto(_)
        )
    }

    pub fn is_unconditional_external_branch(&self) -> bool {
        use JIRInstruction::*;
        matches!(
            self,
            CallDirect(..) | CallIndirect(..) | ReturnDirect(_) | ReturnIndirect(_) | GotoDirect(_) | GotoIndirect(_)
        )
    }

    pub fn is_conditional_branch(&self) -> bool {
        use JIRInstruction::*;
        matches!(self, GotoDirectIf { .. } | GotoIndirectIf { .. } | JCodeGotoIf { .. })
    }

    pub fn operands(&self) -> impl Iterator<Item = &JIRValue> {
        OperandIterator::new(self)
    }

    pub fn operands_mut(&mut self) -> impl Iterator<Item = &mut JIRValue> {
        OperandIteratorMut::new(self)
    }

    pub fn get_output(&self) -> Option<JIRSlice> {
        (match self {
            JIRInstruction::Copy(UnaryArgs { output, .. })
            | JIRInstruction::BitNegate(UnaryArgs { output, .. })
            | JIRInstruction::IntNegate(UnaryArgs { output, .. })
            | JIRInstruction::FloatNegate(UnaryArgs { output, .. })
            | JIRInstruction::BoolNegate(UnaryArgs { output, .. })
            | JIRInstruction::PopCount(UnaryArgs { output, .. })
            | JIRInstruction::LZCount(UnaryArgs { output, .. })
            | JIRInstruction::ZeroExtend(UnaryArgs { output, .. })
            | JIRInstruction::SignExtend(UnaryArgs { output, .. })
            | JIRInstruction::Nan(UnaryArgs { output, .. })
            | JIRInstruction::Abs(UnaryArgs { output, .. })
            | JIRInstruction::Sqrt(UnaryArgs { output, .. })
            | JIRInstruction::IntToFloat(UnaryArgs { output, .. })
            | JIRInstruction::FloatToFloat(UnaryArgs { output, .. })
            | JIRInstruction::Trunc(UnaryArgs { output, .. })
            | JIRInstruction::Ceil(UnaryArgs { output, .. })
            | JIRInstruction::Floor(UnaryArgs { output, .. })
            | JIRInstruction::Round(UnaryArgs { output, .. }) => Some(output),
            JIRInstruction::Load { target, .. } => Some(target),
            JIRInstruction::IntAdd(BinaryArgs { output, .. })
            | JIRInstruction::IntSubtract(BinaryArgs { output, .. })
            | JIRInstruction::IntMul(BinaryArgs { output, .. })
            | JIRInstruction::IntDiv(BinaryArgs { output, .. })
            | JIRInstruction::IntSDiv(BinaryArgs { output, .. })
            | JIRInstruction::IntRemainder(BinaryArgs { output, .. })
            | JIRInstruction::IntSRemainder(BinaryArgs { output, .. })
            | JIRInstruction::IntLeft(BinaryArgs { output, .. })
            | JIRInstruction::IntRight(BinaryArgs { output, .. })
            | JIRInstruction::IntSRight(BinaryArgs { output, .. })
            | JIRInstruction::IntOr(BinaryArgs { output, .. })
            | JIRInstruction::IntAnd(BinaryArgs { output, .. })
            | JIRInstruction::IntXor(BinaryArgs { output, .. })
            | JIRInstruction::IntSLess(BinaryArgs { output, .. })
            | JIRInstruction::IntSGreater(BinaryArgs { output, .. })
            | JIRInstruction::IntSLessEqual(BinaryArgs { output, .. })
            | JIRInstruction::IntSGreaterEqual(BinaryArgs { output, .. })
            | JIRInstruction::IntLess(BinaryArgs { output, .. })
            | JIRInstruction::IntGreater(BinaryArgs { output, .. })
            | JIRInstruction::IntLessEqual(BinaryArgs { output, .. })
            | JIRInstruction::IntGreaterEqual(BinaryArgs { output, .. })
            | JIRInstruction::IntEquals(BinaryArgs { output, .. })
            | JIRInstruction::IntNotEquals(BinaryArgs { output, .. })
            | JIRInstruction::FloatDiv(BinaryArgs { output, .. })
            | JIRInstruction::FloatMul(BinaryArgs { output, .. })
            | JIRInstruction::FloatAdd(BinaryArgs { output, .. })
            | JIRInstruction::FloatSubtract(BinaryArgs { output, .. })
            | JIRInstruction::FloatLess(BinaryArgs { output, .. })
            | JIRInstruction::FloatGreater(BinaryArgs { output, .. })
            | JIRInstruction::FloatLessEqual(BinaryArgs { output, .. })
            | JIRInstruction::FloatGreaterEqual(BinaryArgs { output, .. })
            | JIRInstruction::FloatEquals(BinaryArgs { output, .. })
            | JIRInstruction::FloatNotEquals(BinaryArgs { output, .. })
            | JIRInstruction::BoolOr(BinaryArgs { output, .. })
            | JIRInstruction::BoolAnd(BinaryArgs { output, .. })
            | JIRInstruction::BoolXor(BinaryArgs { output, .. })
            | JIRInstruction::Carry(BinaryArgs { output, .. })
            | JIRInstruction::SignCarry(BinaryArgs { output, .. })
            | JIRInstruction::SignBorrow(BinaryArgs { output, .. }) => Some(output),
            JIRInstruction::CustomOp { output, .. } => output.as_ref(),
            _ => None,
        })
        .copied()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BinaryArgs {
    pub output: JIRSlice,
    pub input_a: JIRValue,
    pub input_b: JIRValue,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UnaryArgs {
    pub output: JIRSlice,
    pub input: JIRValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum JIRInstruction {
    // ------------------------ JIR STRUCTURE HELPERS -------------------------
    BlockStart(JIRBlockId),
    BlockEnd(JIRBlockId),
    InstructionStart(EVAddr),
    InstructionEnd(EVAddr),
    // -------
    // TODO: Host Load
    // TODO: Host Store
    // TODO: Trigger interrupt
    // -------------------------- GENERIC LOAD/STORE --------------------------
    Copy(UnaryArgs),
    Load {
        /// The target slice to load into. This determines the size of the load.
        target: JIRSlice,
        /// The address to load from
        address: JIRValue,
        /// The space the address refers to.
        space: SpaceId,
    },
    Store {
        /// The address to store to.
        address: JIRValue,
        /// The value to be stored. The size of the value determines the size of the load.
        value: JIRValue,
        /// The space the address refers to.
        space: SpaceId,
    },
    // -------------------------- INTEGER ARITHMETIC --------------------------
    // Binary operations
    IntAdd(BinaryArgs),
    IntSubtract(BinaryArgs),
    IntMul(BinaryArgs),
    IntDiv(BinaryArgs),
    IntSDiv(BinaryArgs),
    IntRemainder(BinaryArgs),
    IntSRemainder(BinaryArgs),
    IntLeft(BinaryArgs),
    IntRight(BinaryArgs),
    IntSRight(BinaryArgs),
    IntOr(BinaryArgs),
    IntAnd(BinaryArgs),
    IntXor(BinaryArgs),
    // Comparisons
    IntSLess(BinaryArgs),
    IntSGreater(BinaryArgs),
    IntSLessEqual(BinaryArgs),
    IntSGreaterEqual(BinaryArgs),
    IntLess(BinaryArgs),
    IntGreater(BinaryArgs),
    IntLessEqual(BinaryArgs),
    IntGreaterEqual(BinaryArgs),
    IntEquals(BinaryArgs),
    IntNotEquals(BinaryArgs),
    // Unary operations
    BitNegate(UnaryArgs),
    IntNegate(UnaryArgs),
    // --------------------------- FLOAT ARITHMETIC ---------------------------
    // Binary operations
    FloatDiv(BinaryArgs),
    FloatMul(BinaryArgs),
    FloatAdd(BinaryArgs),
    FloatSubtract(BinaryArgs),
    // Comparisons
    FloatLess(BinaryArgs),
    FloatGreater(BinaryArgs),
    FloatLessEqual(BinaryArgs),
    FloatGreaterEqual(BinaryArgs),
    FloatEquals(BinaryArgs),
    FloatNotEquals(BinaryArgs),
    // Unary operations
    FloatNegate(UnaryArgs),

    // -------------------------- BOOLEAN OPERATIONS --------------------------
    // Binary operations
    BoolOr(BinaryArgs),
    BoolAnd(BinaryArgs),
    BoolXor(BinaryArgs),
    // Unary operations
    BoolNegate(UnaryArgs),
    // ------------------------- BRANCHING OPERATIONS -------------------------
    // TODO: Add space id for jump instructions
    // Same as goto, but semantically different for analysis purposes.
    CallDirect(JIRValue, EVAddr),
    CallIndirect(JIRValue, EVAddr),

    // Same as goto, but semantically different for analysis purposes.
    ReturnDirect(JIRValue),
    ReturnIndirect(JIRValue),

    // Simple Goto
    GotoDirect(JIRValue),
    GotoIndirect(JIRValue),
    GotoDirectIf {
        condition: JIRValue,
        target: JIRValue,
    },
    GotoIndirectIf {
        condition: JIRValue,
        target: JIRValue,
    },
    // Internal branches. JCode internal branches can only jump to other blocks inside a single TargetInstruction
    JCodeGoto(JIRBlockId),
    JCodeGotoIf {
        condition: JIRValue,
        target_block: JIRBlockId,
    },

    // -------------------------- SPECIAL OPERATIONS --------------------------
    PopCount(UnaryArgs),
    LZCount(UnaryArgs),
    ZeroExtend(UnaryArgs),
    SignExtend(UnaryArgs),
    Carry(BinaryArgs),
    SignCarry(BinaryArgs),
    SignBorrow(BinaryArgs),
    Nan(UnaryArgs),
    Abs(UnaryArgs),
    Sqrt(UnaryArgs),
    IntToFloat(UnaryArgs),
    FloatToFloat(UnaryArgs),
    Trunc(UnaryArgs),
    Ceil(UnaryArgs),
    Floor(UnaryArgs),
    Round(UnaryArgs),
    CustomOp {
        output: Option<JIRSlice>,
        input_a: Option<JIRValue>,
        input_b: Option<JIRValue>,
        op_id: CustomOpId,
        extra_args: Rc<Vec<JIRValue>>,
    },
    Nop,
}

struct OperandIterator<'a> {
    instruction: &'a JIRInstruction,
    next_index: usize,
}

impl<'a> OperandIterator<'a> {
    fn new(instruction: &'a JIRInstruction) -> Self {
        Self {
            instruction,
            next_index: 0,
        }
    }
}

struct OperandIteratorMut<'a> {
    instruction: &'a mut JIRInstruction,
    next_index: usize,
}

impl<'a> OperandIteratorMut<'a> {
    fn new(instruction: &'a mut JIRInstruction) -> Self {
        Self {
            instruction,
            next_index: 0,
        }
    }
}

impl<'a> Iterator for OperandIterator<'a> {
    type Item = &'a JIRValue;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.instruction {
            JIRInstruction::Copy(UnaryArgs { input, .. })
            | JIRInstruction::BitNegate(UnaryArgs { input, .. })
            | JIRInstruction::IntNegate(UnaryArgs { input, .. })
            | JIRInstruction::FloatNegate(UnaryArgs { input, .. })
            | JIRInstruction::BoolNegate(UnaryArgs { input, .. })
            | JIRInstruction::PopCount(UnaryArgs { input, .. })
            | JIRInstruction::LZCount(UnaryArgs { input, .. })
            | JIRInstruction::ZeroExtend(UnaryArgs { input, .. })
            | JIRInstruction::SignExtend(UnaryArgs { input, .. })
            | JIRInstruction::Nan(UnaryArgs { input, .. })
            | JIRInstruction::Abs(UnaryArgs { input, .. })
            | JIRInstruction::Sqrt(UnaryArgs { input, .. })
            | JIRInstruction::IntToFloat(UnaryArgs { input, .. })
            | JIRInstruction::FloatToFloat(UnaryArgs { input, .. })
            | JIRInstruction::Trunc(UnaryArgs { input, .. })
            | JIRInstruction::Ceil(UnaryArgs { input, .. })
            | JIRInstruction::Floor(UnaryArgs { input, .. })
            | JIRInstruction::Round(UnaryArgs { input, .. })
                if self.next_index == 0 =>
            {
                Some(input)
            }
            JIRInstruction::Load { address, .. } if self.next_index == 0 => Some(address),
            JIRInstruction::Store { address, value, .. } => match self.next_index {
                0 => Some(address),
                1 => Some(value),
                _ => None,
            },
            JIRInstruction::IntAdd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSubtract(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntMul(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntRemainder(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSRemainder(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLeft(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntRight(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSRight(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntOr(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntAnd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntXor(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntNotEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatMul(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatAdd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatSubtract(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatNotEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolOr(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolAnd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolXor(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::Carry(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::SignCarry(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::SignBorrow(BinaryArgs { input_a, input_b, .. }) => match self.next_index {
                0 => Some(input_a),
                1 => Some(input_b),
                _ => None,
            },
            JIRInstruction::GotoDirect(target)
            | JIRInstruction::GotoIndirect(target)
            | JIRInstruction::ReturnDirect(target)
            | JIRInstruction::ReturnIndirect(target)
                if self.next_index == 0 =>
            {
                Some(target)
            }
            JIRInstruction::CallDirect(target, _) | JIRInstruction::CallIndirect(target, _) if self.next_index == 0 => {
                Some(target)
            }
            JIRInstruction::GotoDirectIf { condition, target }
            | JIRInstruction::GotoIndirectIf { condition, target } => match self.next_index {
                0 => Some(condition),
                1 => Some(target),
                _ => None,
            },
            JIRInstruction::JCodeGotoIf { condition, .. } if self.next_index == 0 => Some(condition),
            JIRInstruction::CustomOp {
                input_a,
                input_b,
                extra_args,
                ..
            } => {
                if self.next_index == 0 {
                    input_a.as_ref()
                } else if self.next_index == 1 {
                    input_b.as_ref()
                } else if self.next_index < extra_args.len() + 2 {
                    Some(&extra_args[self.next_index - 2])
                } else {
                    None
                }
            }
            _ => None,
        };
        self.next_index += 1;
        result
    }
}

impl<'a> Iterator for OperandIteratorMut<'a> {
    type Item = &'a mut JIRValue;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.instruction {
            JIRInstruction::Copy(UnaryArgs { input, .. })
            | JIRInstruction::BitNegate(UnaryArgs { input, .. })
            | JIRInstruction::IntNegate(UnaryArgs { input, .. })
            | JIRInstruction::FloatNegate(UnaryArgs { input, .. })
            | JIRInstruction::BoolNegate(UnaryArgs { input, .. })
            | JIRInstruction::PopCount(UnaryArgs { input, .. })
            | JIRInstruction::LZCount(UnaryArgs { input, .. })
            | JIRInstruction::ZeroExtend(UnaryArgs { input, .. })
            | JIRInstruction::SignExtend(UnaryArgs { input, .. })
            | JIRInstruction::Nan(UnaryArgs { input, .. })
            | JIRInstruction::Abs(UnaryArgs { input, .. })
            | JIRInstruction::Sqrt(UnaryArgs { input, .. })
            | JIRInstruction::IntToFloat(UnaryArgs { input, .. })
            | JIRInstruction::FloatToFloat(UnaryArgs { input, .. })
            | JIRInstruction::Trunc(UnaryArgs { input, .. })
            | JIRInstruction::Ceil(UnaryArgs { input, .. })
            | JIRInstruction::Floor(UnaryArgs { input, .. })
            | JIRInstruction::Round(UnaryArgs { input, .. })
                if self.next_index == 0 =>
            {
                Some(input)
            }
            JIRInstruction::Load { address, .. } if self.next_index == 0 => Some(address),
            JIRInstruction::Store { address, value, .. } => match self.next_index {
                0 => Some(address),
                1 => Some(value),
                _ => None,
            },
            JIRInstruction::IntAdd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSubtract(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntMul(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntRemainder(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSRemainder(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLeft(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntRight(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSRight(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntOr(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntAnd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntXor(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntSGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::IntNotEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatDiv(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatMul(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatAdd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatSubtract(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatLess(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatGreater(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatLessEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatGreaterEqual(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::FloatNotEquals(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolOr(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolAnd(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::BoolXor(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::Carry(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::SignCarry(BinaryArgs { input_a, input_b, .. })
            | JIRInstruction::SignBorrow(BinaryArgs { input_a, input_b, .. }) => match self.next_index {
                0 => Some(input_a),
                1 => Some(input_b),
                _ => None,
            },
            JIRInstruction::GotoDirect(target)
            | JIRInstruction::GotoIndirect(target)
            | JIRInstruction::ReturnDirect(target)
            | JIRInstruction::ReturnIndirect(target)
                if self.next_index == 0 =>
            {
                Some(target)
            }
            JIRInstruction::CallDirect(target, _) | JIRInstruction::CallIndirect(target, _) if self.next_index == 0 => {
                Some(target)
            }
            JIRInstruction::GotoDirectIf { condition, target }
            | JIRInstruction::GotoIndirectIf { condition, target } => match self.next_index {
                0 => Some(condition),
                1 => Some(target),
                _ => None,
            },
            JIRInstruction::JCodeGotoIf { condition, .. } if self.next_index == 0 => Some(condition),
            JIRInstruction::CustomOp {
                input_a,
                input_b,
                extra_args,
                ..
            } => {
                if self.next_index == 0 {
                    input_a.as_mut()
                } else if self.next_index == 1 {
                    input_b.as_mut()
                } else if self.next_index < extra_args.len() + 2 {
                    Some(&mut Rc::make_mut(extra_args)[self.next_index - 2])
                } else {
                    None
                }
            }
            _ => None,
        };
        self.next_index += 1;

        let ptr: Option<*mut JIRValue> = result.map(|res| res as _);

        // SAFETY:
        // - since `self.next_index` is always advanced, no mutable reference to
        // the same location is ever returned more than once
        unsafe { ptr.map(|p| p.as_mut().expect("Pointer is guaranteed to not be null")) }
    }
}
