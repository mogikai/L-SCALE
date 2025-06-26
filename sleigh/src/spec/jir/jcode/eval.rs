use crate::spec::SpaceId;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::values::{
    JIRConst,
    JIRValue,
};

use super::JIRInstruction;

impl JIRInstruction {
    #[allow(clippy::cognitive_complexity)]
    pub fn evaluate(
        &self,
        const_space_id: SpaceId,
    ) -> Option<JIRConst> {
        // Closure to safely apply size-based masking
        let apply_mask = |value: u128, size: JIRSize| {
            let mask = if size as u32 >= 16 {
                u128::MAX // For size >= 16, use full mask since shift would overflow
            } else {
                (1u128 << (size * 8)) - 1 // Normal case for size < 16
            };
            value & mask
        };

        match self {
            // Integer binary operations
            Self::IntAdd(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val.wrapping_add(b_val), size), size))
                } else {
                    None
                }
            }
            Self::IntSubtract(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val.wrapping_sub(b_val), size), size))
                } else {
                    None
                }
            }
            Self::IntMul(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val.wrapping_mul(b_val), size), size))
                } else {
                    None
                }
            }
            Self::IntDiv(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    if b.value() == 0 {
                        return None; // Division by zero
                    }
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val.wrapping_div(b_val), size), size))
                } else {
                    None
                }
            }
            Self::IntSDiv(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    if b.value() == 0 {
                        return None; // Division by zero
                    }

                    let size = a.size();
                    let bit_size = size * 8;

                    // Sign-extend to i128
                    let sign_bit_a = (a.value() >> (bit_size - 1)) & 1;
                    let sign_bit_b = (b.value() >> (bit_size - 1)) & 1;

                    let a_val = if sign_bit_a == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (a.value() | mask) as i128
                    } else {
                        a.value() as i128
                    };

                    let b_val = if sign_bit_b == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (b.value() | mask) as i128
                    } else {
                        b.value() as i128
                    };

                    let result = (a_val / b_val) as u128;
                    Some(JIRConst::new(result % (1u128 << bit_size), size))
                } else {
                    None
                }
            }
            Self::IntRemainder(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    if b.value() == 0 {
                        return None; // Division by zero
                    }
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val % b_val, size), size))
                } else {
                    None
                }
            }
            Self::IntSRemainder(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    if b.value() == 0 {
                        return None; // Division by zero
                    }

                    let size = a.size();
                    let bit_size = size * 8;

                    // Sign-extend to i128
                    let sign_bit_a = (a.value() >> (bit_size - 1)) & 1;
                    let sign_bit_b = (b.value() >> (bit_size - 1)) & 1;

                    let a_val = if sign_bit_a == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (a.value() | mask) as i128
                    } else {
                        a.value() as i128
                    };

                    let b_val = if sign_bit_b == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (b.value() | mask) as i128
                    } else {
                        b.value() as i128
                    };

                    let result = (a_val % b_val) as u128;
                    Some(JIRConst::new(apply_mask(result, size), size))
                } else {
                    None
                }
            }
            Self::IntLeft(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    assert!(b_val < (size as u128 * 8), "Shift amount {b_val} is too large for {size}-byte value");
                    let shift = b_val as u32;
                    Some(JIRConst::new(apply_mask(a_val << shift, size), size))
                } else {
                    None
                }
            }
            Self::IntRight(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    assert!(b_val < (size as u128 * 8), "Shift amount {b_val} is too large for {size}-byte value");
                    let shift = b_val as u32;
                    Some(JIRConst::new(apply_mask(a_val >> shift, size), size))
                } else {
                    None
                }
            }
            Self::IntSRight(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let bit_size = size * 8;
                    let a_val = a.value();
                    let b_val = b.value();

                    assert!(b_val < bit_size as u128, "Shift amount {b_val} is too large for {size}-byte value");
                    let shift = b_val as u32;

                    // Check if sign bit is set
                    let sign_bit = (a_val >> (bit_size - 1)) & 1;

                    let result = if sign_bit == 1 {
                        // Set all bits above the sign bit to 1
                        let mask = !((1u128 << bit_size) - 1);
                        let sign_extended = a_val | mask;
                        (sign_extended >> shift) & ((1u128 << bit_size) - 1)
                    } else {
                        a_val >> shift
                    };

                    Some(JIRConst::new(result, size))
                } else {
                    None
                }
            }
            Self::IntOr(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val | b_val, size), size))
                } else {
                    None
                }
            }
            Self::IntAnd(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val & b_val, size), size))
                } else {
                    None
                }
            }
            Self::IntXor(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();
                    Some(JIRConst::new(apply_mask(a_val ^ b_val, size), size))
                } else {
                    None
                }
            }

            // Integer comparison operations
            Self::IntEquals(args)
            | Self::IntNotEquals(args)
            | Self::IntLess(args)
            | Self::IntGreater(args)
            | Self::IntLessEqual(args)
            | Self::IntGreaterEqual(args)
            | Self::IntSLess(args)
            | Self::IntSGreater(args)
            | Self::IntSLessEqual(args)
            | Self::IntSGreaterEqual(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let a_val = a.value();
                    let b_val = b.value();

                    // For signed comparisons
                    let bit_size = a.size() * 8;
                    let sign_bit_a = (a_val >> (bit_size - 1)) & 1;
                    let sign_bit_b = (b_val >> (bit_size - 1)) & 1;

                    let a_signed = if sign_bit_a == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (a_val | mask) as i128
                    } else {
                        a_val as i128
                    };

                    let b_signed = if sign_bit_b == 1 {
                        let mask = !((1u128 << bit_size) - 1);
                        (b_val | mask) as i128
                    } else {
                        b_val as i128
                    };

                    let result = match self {
                        Self::IntEquals(_) => (a_val == b_val) as u128,
                        Self::IntNotEquals(_) => (a_val != b_val) as u128,
                        Self::IntLess(_) => (a_val < b_val) as u128,
                        Self::IntGreater(_) => (a_val > b_val) as u128,
                        Self::IntLessEqual(_) => (a_val <= b_val) as u128,
                        Self::IntGreaterEqual(_) => (a_val >= b_val) as u128,
                        Self::IntSLess(_) => (a_signed < b_signed) as u128,
                        Self::IntSGreater(_) => (a_signed > b_signed) as u128,
                        Self::IntSLessEqual(_) => (a_signed <= b_signed) as u128,
                        Self::IntSGreaterEqual(_) => (a_signed >= b_signed) as u128,
                        _ => unreachable!(),
                    };

                    // Boolean results use 1-byte size
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }

            // Float operations
            Self::FloatAdd(args) | Self::FloatSubtract(args) | Self::FloatMul(args) | Self::FloatDiv(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();

                    // We only support 4-byte (f32) and 8-byte (f64) floats
                    if size != 4 && size != 8 {
                        return None;
                    }

                    let result = match size {
                        4 => {
                            let a_f32 = f32::from_bits(a.value() as u32);
                            let b_f32 = f32::from_bits(b.value() as u32);

                            let result_f32 = match self {
                                Self::FloatAdd(_) => a_f32 + b_f32,
                                Self::FloatSubtract(_) => a_f32 - b_f32,
                                Self::FloatMul(_) => a_f32 * b_f32,
                                Self::FloatDiv(_) => {
                                    if b_f32 == 0.0 {
                                        return None; // Division by zero
                                    }
                                    a_f32 / b_f32
                                }
                                _ => unreachable!(),
                            };

                            u128::from(result_f32.to_bits())
                        }
                        8 => {
                            let a_f64 = f64::from_bits(a.value() as u64);
                            let b_f64 = f64::from_bits(b.value() as u64);

                            let result_f64 = match self {
                                Self::FloatAdd(_) => a_f64 + b_f64,
                                Self::FloatSubtract(_) => a_f64 - b_f64,
                                Self::FloatMul(_) => a_f64 * b_f64,
                                Self::FloatDiv(_) => {
                                    if b_f64 == 0.0 {
                                        return None; // Division by zero
                                    }
                                    a_f64 / b_f64
                                }
                                _ => unreachable!(),
                            };

                            u128::from(result_f64.to_bits())
                        }
                        _ => unreachable!(),
                    };

                    Some(JIRConst::new(result, size))
                } else {
                    None
                }
            }

            // Float comparison operations
            Self::FloatEquals(args)
            | Self::FloatNotEquals(args)
            | Self::FloatLess(args)
            | Self::FloatGreater(args)
            | Self::FloatLessEqual(args)
            | Self::FloatGreaterEqual(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();

                    // We only support 4-byte (f32) and 8-byte (f64) floats
                    if size != 4 && size != 8 {
                        return None;
                    }

                    let result = match size {
                        4 => {
                            let a_f32 = f32::from_bits(a.value() as u32);
                            let b_f32 = f32::from_bits(b.value() as u32);

                            match self {
                                Self::FloatEquals(_) => (a_f32 == b_f32) as u128,
                                Self::FloatNotEquals(_) => (a_f32 != b_f32) as u128,
                                Self::FloatLess(_) => (a_f32 < b_f32) as u128,
                                Self::FloatGreater(_) => (a_f32 > b_f32) as u128,
                                Self::FloatLessEqual(_) => (a_f32 <= b_f32) as u128,
                                Self::FloatGreaterEqual(_) => (a_f32 >= b_f32) as u128,
                                _ => unreachable!(),
                            }
                        }
                        8 => {
                            let a_f64 = f64::from_bits(a.value() as u64);
                            let b_f64 = f64::from_bits(b.value() as u64);

                            match self {
                                Self::FloatEquals(_) => (a_f64 == b_f64) as u128,
                                Self::FloatNotEquals(_) => (a_f64 != b_f64) as u128,
                                Self::FloatLess(_) => (a_f64 < b_f64) as u128,
                                Self::FloatGreater(_) => (a_f64 > b_f64) as u128,
                                Self::FloatLessEqual(_) => (a_f64 <= b_f64) as u128,
                                Self::FloatGreaterEqual(_) => (a_f64 >= b_f64) as u128,
                                _ => unreachable!(),
                            }
                        }
                        _ => unreachable!(),
                    };

                    // Boolean results use 1-byte size
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }

            // Boolean operations
            Self::BoolOr(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let result = ((a.value() != 0) || (b.value() != 0)) as u128;
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }
            Self::BoolAnd(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let result = ((a.value() != 0) && (b.value() != 0)) as u128;
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }
            Self::BoolXor(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let result = ((a.value() != 0) != (b.value() != 0)) as u128;
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }

            // Unary operations
            Self::Copy(args) => {
                if let JIRValue::Const(a) = args.input {
                    Some(a)
                } else {
                    None
                }
            }
            Self::BitNegate(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();
                    let val = a.value();
                    Some(JIRConst::new(apply_mask(!val, size), size))
                } else {
                    None
                }
            }
            Self::IntNegate(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();
                    let val = a.value();
                    Some(JIRConst::new(val.wrapping_neg(), size))
                } else {
                    None
                }
            }
            Self::FloatNegate(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();

                    // We only support 4-byte (f32) and 8-byte (f64) floats
                    if size != 4 && size != 8 {
                        return None;
                    }

                    let result = match size {
                        4 => {
                            let a_f32 = f32::from_bits(a.value() as u32);
                            u128::from((-a_f32).to_bits())
                        }
                        8 => {
                            let a_f64 = f64::from_bits(a.value() as u64);
                            u128::from((-a_f64).to_bits())
                        }
                        _ => unreachable!(),
                    };

                    Some(JIRConst::new(result, size))
                } else {
                    None
                }
            }
            Self::BoolNegate(args) => {
                if let JIRValue::Const(a) = args.input {
                    let result = (a.value() == 0) as u128;
                    Some(JIRConst::new(result, 1))
                } else {
                    None
                }
            }

            // Extension operations
            Self::ZeroExtend(args) => {
                if let JIRValue::Const(a) = args.input {
                    let output_size = args.output.size();
                    let a_val = a.value();
                    Some(JIRConst::new(apply_mask(a_val, output_size), output_size))
                } else {
                    None
                }
            }
            Self::SignExtend(args) => {
                if let JIRValue::Const(a) = args.input {
                    let output_size = args.output.size();
                    let a_val = a.value();
                    let input_size = a.size();

                    // Check if the highest bit is set (sign bit)
                    let sign_bit_mask = 1u128 << (input_size * 8 - 1);
                    let is_negative = (a_val & sign_bit_mask) != 0;

                    let result = if is_negative {
                        // Fill the higher bits with 1s to extend the sign
                        let input_mask = (1u128 << (input_size * 8)) - 1;
                        let extension_mask = !input_mask & ((1u128 << (output_size * 8)) - 1);
                        a_val | extension_mask
                    } else {
                        // Zero extend
                        a_val
                    };

                    Some(JIRConst::new(result, output_size))
                } else {
                    None
                }
            }
            Self::Trunc(args) => {
                if let JIRValue::Const(a) = args.input {
                    let output_size = args.output.size();
                    let a_val = a.value();
                    let mask = (1u128 << (output_size * 8)) - 1;
                    Some(JIRConst::new(a_val & mask, output_size))
                } else {
                    None
                }
            }

            // Special functions
            Self::PopCount(args) => {
                if let JIRValue::Const(a) = args.input {
                    let val = a.value();
                    let count = val.count_ones() as u128;
                    Some(JIRConst::new(count, 1))
                } else {
                    None
                }
            }
            Self::LZCount(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();
                    let val = a.value();
                    let count = val.leading_zeros() as u128 - (128 - (size * 8) as u32) as u128;
                    Some(JIRConst::new(count, 1))
                } else {
                    None
                }
            }
            Self::Abs(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();

                    if size == 4 {
                        let a_f32 = f32::from_bits(a.value() as u32);
                        let result = a_f32.abs();
                        Some(JIRConst::new(u128::from(result.to_bits()), size))
                    } else if size == 8 {
                        let a_f64 = f64::from_bits(a.value() as u64);
                        let result = a_f64.abs();
                        Some(JIRConst::new(u128::from(result.to_bits()), size))
                    } else {
                        // Integer absolute value
                        let bit_size = size * 8;
                        let sign_bit = (a.value() >> (bit_size - 1)) & 1;

                        if sign_bit == 1 {
                            // Negate the value for negative numbers
                            let mask = (1u128 << bit_size) - 1;
                            let neg = (!a.value() + 1) & mask;
                            Some(JIRConst::new(neg, size))
                        } else {
                            Some(a)
                        }
                    }
                } else {
                    None
                }
            }
            Self::Sqrt(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();

                    if size == 4 {
                        let a_f32 = f32::from_bits(a.value() as u32);
                        if a_f32 < 0.0 {
                            return None; // Sqrt of negative number
                        }
                        let result = a_f32.sqrt();
                        Some(JIRConst::new(u128::from(result.to_bits()), size))
                    } else if size == 8 {
                        let a_f64 = f64::from_bits(a.value() as u64);
                        if a_f64 < 0.0 {
                            return None; // Sqrt of negative number
                        }
                        let result = a_f64.sqrt();
                        Some(JIRConst::new(u128::from(result.to_bits()), size))
                    } else {
                        // Integer sqrt approximation
                        let result = (a.value() as f64).sqrt() as u128;
                        Some(JIRConst::new(result, size))
                    }
                } else {
                    None
                }
            }
            Self::Nan(args) => {
                if let JIRValue::Const(a) = args.input {
                    let size = a.size();

                    let is_nan = if size == 4 {
                        let a_f32 = f32::from_bits(a.value() as u32);
                        a_f32.is_nan()
                    } else if size == 8 {
                        let a_f64 = f64::from_bits(a.value() as u64);
                        a_f64.is_nan()
                    } else {
                        false // Non-float types can't be NaN
                    };

                    Some(JIRConst::new(is_nan as u128, 1))
                } else {
                    None
                }
            }
            Self::Carry(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let a_val = a.value();
                    let b_val = b.value();

                    let max = 1u128 << (size * 8);
                    let result = a_val.wrapping_add(b_val);
                    let carry = (result < a_val || result >= max) as u128;

                    Some(JIRConst::new(carry, 1))
                } else {
                    None
                }
            }
            Self::SignCarry(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let bit_size = size * 8;
                    let a_val = a.value();
                    let b_val = b.value();

                    let sign_bit_mask = 1u128 << (bit_size - 1);
                    let result = a_val.wrapping_add(b_val);

                    // Check if the sign bit of the result is different from
                    // what we'd expect by looking at the operands' signs
                    let a_sign = (a_val & sign_bit_mask) != 0;
                    let b_sign = (b_val & sign_bit_mask) != 0;
                    let res_sign = (result & sign_bit_mask) != 0;

                    // Signed overflow occurs when:
                    // - Both inputs are positive but result is negative
                    // - Both inputs are negative but result is positive
                    let carry = ((a_sign == b_sign) && (a_sign != res_sign)) as u128;

                    Some(JIRConst::new(carry, 1))
                } else {
                    None
                }
            }
            Self::SignBorrow(args) => {
                if let (JIRValue::Const(a), JIRValue::Const(b)) = (args.input_a, args.input_b) {
                    let size = a.size();
                    let bit_size = size * 8;
                    let a_val = a.value();
                    let b_val = b.value();

                    let sign_bit_mask = 1u128 << (bit_size - 1);

                    // Sign bit for the a
                    let a_sign = (a_val & sign_bit_mask) != 0;
                    let b_sign = (b_val & sign_bit_mask) != 0;

                    // Check for underflow
                    // When a positive number is substracted from a negative one,
                    // it may cause underflow in the positive range
                    // Same, when a negative number is substracted from a positive one
                    let borrow = ((a_sign != b_sign) && (a_val < b_val)) as u128;

                    Some(JIRConst::new(borrow, 1))
                } else {
                    None
                }
            }
            Self::Load { target, address, space } => {
                if *space == const_space_id {
                    let JIRValue::Const(const_addr) = address else {
                        panic!("Const space addresses must be const");
                    };

                    Some(JIRConst::new(const_addr.value(), target.size()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::SpaceId;
    use crate::spec::jir::common::JIROffset;
    use crate::spec::jir::jcode::{
        BinaryArgs,
        UnaryArgs,
    };
    use crate::spec::jir::values::{
        GroupId,
        JIRConst,
        JIRSlice,
        JIRValue,
        VarnodeGroupId,
    };
    use mogi_utilities::index_map::MapKey;
    use std::{
        f32,
        f64,
    };

    // Helper function for creating and evaluating binary operations
    fn test_binary_op(
        op_fn: fn(BinaryArgs) -> JIRInstruction,
        a: u128,
        b: u128,
        expected: Option<u128>,
        size: JIRSize,
        result_size: Option<JIRSize>,
    ) {
        let result_size = result_size.unwrap_or(size);
        let args = BinaryArgs {
            input_a: JIRValue::Const(JIRConst::new(a, size)),
            input_b: JIRValue::Const(JIRConst::new(b, size)),
            output: JIRSlice::new(
                SpaceId::try_from_index(1).unwrap(),
                GroupId::try_from_index(1).unwrap(),
                JIROffset::try_from(0u64).unwrap(),
                result_size,
            ),
        };

        let inst = op_fn(args);
        let evaluated = inst.evaluate(SpaceId::try_from_index(1).unwrap());

        match expected {
            Some(expected_val) => {
                assert!(evaluated.is_some(), "Expected Some value, got None");
                assert_eq!(evaluated.unwrap(), JIRConst::new(expected_val, result_size));
            }
            None => assert!(evaluated.is_none(), "Expected None, got Some value"),
        }
    }

    // Helper function for creating and evaluating unary operations
    fn test_unary_op(
        op_fn: fn(UnaryArgs) -> JIRInstruction,
        a: u128,
        expected: Option<u128>,
        input_size: JIRSize,
        output_size: Option<JIRSize>,
    ) {
        let result_size = output_size.unwrap_or(input_size);

        let args = UnaryArgs {
            input: JIRValue::Const(JIRConst::new(a, input_size)),
            output: if let Some(output_size) = output_size {
                JIRSlice::new(
                    SpaceId::try_from_index(1).unwrap(),
                    GroupId::try_from_index(1).unwrap(),
                    JIROffset::try_from(0u64).unwrap(),
                    output_size,
                )
            } else {
                JIRSlice::new(
                    SpaceId::try_from_index(1).unwrap(),
                    GroupId::try_from_index(1).unwrap(),
                    JIROffset::try_from(0u64).unwrap(),
                    JIRSize::try_from(0u64).unwrap(),
                )
            },
        };

        let inst = op_fn(args);
        let evaluated = inst.evaluate(SpaceId::try_from_index(1).unwrap());

        match expected {
            Some(expected_val) => {
                assert!(evaluated.is_some(), "Expected Some value, got None");
                assert_eq!(evaluated.unwrap(), JIRConst::new(expected_val, result_size));
            }
            None => assert!(evaluated.is_none(), "Expected None, got Some value"),
        }
    }

    //==================================================
    // INTEGER BINARY OPERATIONS TESTS
    //==================================================

    #[test]
    fn test_int_addition() {
        // Basic addition
        test_binary_op(JIRInstruction::IntAdd, 5, 3, Some(8), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntAdd, 5, 3, Some(8), 2, None);
        test_binary_op(JIRInstruction::IntAdd, 5, 3, Some(8), 4, None);
        test_binary_op(JIRInstruction::IntAdd, 5, 3, Some(8), 8, None);

        // Overflow cases
        test_binary_op(JIRInstruction::IntAdd, 255, 1, Some(0), 1, None);
        test_binary_op(JIRInstruction::IntAdd, 65535, 1, Some(0), 2, None);
        test_binary_op(JIRInstruction::IntAdd, 0xFFFFFFFF, 1, Some(0), 4, None);

        // Large values
        test_binary_op(JIRInstruction::IntAdd, 0x7FFFFFFF, 1, Some(0x80000000), 4, None);
    }

    #[test]
    fn test_int_subtraction() {
        // Basic subtraction
        test_binary_op(JIRInstruction::IntSubtract, 5, 3, Some(2), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntSubtract, 5, 3, Some(2), 2, None);
        test_binary_op(JIRInstruction::IntSubtract, 5, 3, Some(2), 4, None);
        test_binary_op(JIRInstruction::IntSubtract, 5, 3, Some(2), 8, None);

        // Underflow cases
        test_binary_op(JIRInstruction::IntSubtract, 0, 1, Some(255), 1, None);
        test_binary_op(JIRInstruction::IntSubtract, 0, 1, Some(65535), 2, None);
        test_binary_op(JIRInstruction::IntSubtract, 0, 1, Some(0xFFFFFFFF), 4, None);

        // Specific cases
        test_binary_op(JIRInstruction::IntSubtract, 0x80000000, 1, Some(0x7FFFFFFF), 4, None);
    }

    #[test]
    fn test_int_multiplication() {
        // Basic multiplication
        test_binary_op(JIRInstruction::IntMul, 5, 3, Some(15), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntMul, 5, 3, Some(15), 2, None);
        test_binary_op(JIRInstruction::IntMul, 5, 3, Some(15), 4, None);
        test_binary_op(JIRInstruction::IntMul, 5, 3, Some(15), 8, None);

        // Overflow cases
        test_binary_op(JIRInstruction::IntMul, 16, 16, Some(0), 1, None);
        test_binary_op(JIRInstruction::IntMul, 0x100, 0x100, Some(0), 2, None);

        // Large values with overflow
        test_binary_op(JIRInstruction::IntMul, 0x10000, 0x10000, Some(0), 4, None);

        // Edge case - multiplication by zero
        test_binary_op(JIRInstruction::IntMul, 42, 0, Some(0), 4, None);
        test_binary_op(JIRInstruction::IntMul, 0, 42, Some(0), 4, None);
    }

    #[test]
    fn test_int_division() {
        // Basic division
        test_binary_op(JIRInstruction::IntDiv, 15, 3, Some(5), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntDiv, 15, 3, Some(5), 2, None);
        test_binary_op(JIRInstruction::IntDiv, 15, 3, Some(5), 4, None);
        test_binary_op(JIRInstruction::IntDiv, 15, 3, Some(5), 8, None);

        // Remainder cases
        test_binary_op(JIRInstruction::IntDiv, 16, 3, Some(5), 1, None);
        test_binary_op(JIRInstruction::IntDiv, 17, 3, Some(5), 1, None);

        // Edge case - division by zero should return None
        test_binary_op(JIRInstruction::IntDiv, 42, 0, None, 4, None);

        // Edge case - dividing zero
        test_binary_op(JIRInstruction::IntDiv, 0, 42, Some(0), 4, None);
    }

    #[test]
    fn test_int_signed_division() {
        // Basic signed division
        test_binary_op(JIRInstruction::IntSDiv, 15, 3, Some(5), 1, None);

        // Negative values (in 8-bit two's complement)
        // -128 / 2 = -64 (which is 0xC0 or 192 in 8-bit)
        test_binary_op(JIRInstruction::IntSDiv, 0x80, 2, Some(0xC0), 1, None);

        // -1 / 2 = 0 with truncation (which is 0xFF or 255 in 8-bit)
        test_binary_op(JIRInstruction::IntSDiv, 0xFF, 2, Some(0), 1, None);

        // Edge case - division by zero should return None
        test_binary_op(JIRInstruction::IntSDiv, 42, 0, None, 4, None);

        // Edge case - dividing zero
        test_binary_op(JIRInstruction::IntSDiv, 0, 42, Some(0), 4, None);

        // Special case: INT_MIN / -1 (this would overflow in two's complement)
        // In 16-bit: -32768 / -1 = 32768, which doesn't fit in 16-bit signed
        test_binary_op(JIRInstruction::IntSDiv, 0x8000, 0xFFFF, Some(0x8000), 2, None);
    }

    #[test]
    fn test_int_remainder() {
        // Basic remainder
        test_binary_op(JIRInstruction::IntRemainder, 17, 5, Some(2), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntRemainder, 17, 5, Some(2), 2, None);
        test_binary_op(JIRInstruction::IntRemainder, 17, 5, Some(2), 4, None);
        test_binary_op(JIRInstruction::IntRemainder, 17, 5, Some(2), 8, None);

        // Edge cases
        test_binary_op(JIRInstruction::IntRemainder, 5, 5, Some(0), 1, None);
        test_binary_op(JIRInstruction::IntRemainder, 0, 5, Some(0), 1, None);

        // Division by zero should return None
        test_binary_op(JIRInstruction::IntRemainder, 42, 0, None, 4, None);
    }

    #[test]
    fn test_int_signed_remainder() {
        // Basic signed remainder
        test_binary_op(JIRInstruction::IntSRemainder, 17, 5, Some(2), 1, None);

        // Negative values in 8-bit two's complement
        // -128 % 5 = -3 (0xFD in 8-bit, which is 253)
        test_binary_op(JIRInstruction::IntSRemainder, 0x80, 5, Some(0xFD), 1, None);

        // -1 % 5 = -1 (0xFF in 8-bit, which is 255)
        test_binary_op(JIRInstruction::IntSRemainder, 0xFF, 5, Some(0xFF), 1, None);

        // Edge cases
        test_binary_op(JIRInstruction::IntSRemainder, 5, 5, Some(0), 1, None);
        test_binary_op(JIRInstruction::IntSRemainder, 0, 5, Some(0), 1, None);

        // Division by zero should return None
        test_binary_op(JIRInstruction::IntSRemainder, 42, 0, None, 4, None);
    }

    #[test]
    fn test_int_shift_left() {
        // Basic left shift
        test_binary_op(JIRInstruction::IntLeft, 0x01, 2, Some(0x04), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntLeft, 0x01, 2, Some(0x04), 2, None);
        test_binary_op(JIRInstruction::IntLeft, 0x01, 2, Some(0x04), 4, None);
        test_binary_op(JIRInstruction::IntLeft, 0x01, 2, Some(0x04), 8, None);

        // Shift with overflow
        test_binary_op(JIRInstruction::IntLeft, 0x40, 1, Some(0x80), 1, None);
        test_binary_op(JIRInstruction::IntLeft, 0x80, 1, Some(0x00), 1, None);
    }

    #[test]
    fn test_int_shift_right() {
        // Basic right shift
        test_binary_op(JIRInstruction::IntRight, 0x08, 2, Some(0x02), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntRight, 0x08, 2, Some(0x02), 2, None);
        test_binary_op(JIRInstruction::IntRight, 0x08, 2, Some(0x02), 4, None);
        test_binary_op(JIRInstruction::IntRight, 0x08, 2, Some(0x02), 8, None);

        // Shift with loss of bits
        test_binary_op(JIRInstruction::IntRight, 0x05, 1, Some(0x02), 1, None);

        // Shift of sign bit (logical, so doesn't extend)
        test_binary_op(JIRInstruction::IntRight, 0x80, 1, Some(0x40), 1, None);
    }

    #[test]
    fn test_int_shift_right_arithmetic() {
        // Basic arithmetic right shift with positive number
        test_binary_op(JIRInstruction::IntSRight, 0x08, 2, Some(0x02), 1, None);

        // Arithmetic right shift with negative number (sign bit preserved)
        test_binary_op(JIRInstruction::IntSRight, 0x80, 1, Some(0xC0), 1, None);
        test_binary_op(JIRInstruction::IntSRight, 0x80, 2, Some(0xE0), 1, None);
        test_binary_op(JIRInstruction::IntSRight, 0x80, 3, Some(0xF0), 1, None);

        // Test with different sizes
        test_binary_op(JIRInstruction::IntSRight, 0x8000, 1, Some(0xC000), 2, None);
        test_binary_op(JIRInstruction::IntSRight, 0x80000000, 1, Some(0xC0000000), 4, None);
    }

    #[test]
    fn test_int_bitwise_ops() {
        // AND operation
        test_binary_op(JIRInstruction::IntAnd, 0b1100, 0b1010, Some(0b1000), 1, None);
        test_binary_op(JIRInstruction::IntAnd, 0xFFFF, 0xFF00, Some(0xFF00), 2, None);

        // OR operation
        test_binary_op(JIRInstruction::IntOr, 0b1100, 0b1010, Some(0b1110), 1, None);
        test_binary_op(JIRInstruction::IntOr, 0xFF00, 0x00FF, Some(0xFFFF), 2, None);

        // XOR operation
        test_binary_op(JIRInstruction::IntXor, 0b1100, 0b1010, Some(0b0110), 1, None);
        test_binary_op(JIRInstruction::IntXor, 0xFF00, 0xFFFF, Some(0x00FF), 2, None);
    }

    //==================================================
    // INTEGER COMPARISON TESTS
    //==================================================

    #[test]
    fn test_int_equality_comparison() {
        // Equal values
        test_binary_op(JIRInstruction::IntEquals, 5, 5, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntEquals, 0xFF, 0xFF, Some(1), 2, Some(1));

        // Unequal values
        test_binary_op(JIRInstruction::IntEquals, 5, 3, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::IntEquals, 0xFF, 0xFE, Some(0), 2, Some(1));

        // Not equals
        test_binary_op(JIRInstruction::IntNotEquals, 5, 3, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntNotEquals, 5, 5, Some(0), 1, Some(1));
    }

    #[test]
    fn test_int_order_comparison() {
        // Less than
        test_binary_op(JIRInstruction::IntLess, 3, 5, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntLess, 5, 5, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::IntLess, 5, 3, Some(0), 1, Some(1));

        // Greater than
        test_binary_op(JIRInstruction::IntGreater, 5, 3, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntGreater, 5, 5, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::IntGreater, 3, 5, Some(0), 1, Some(1));

        // Less than or equal
        test_binary_op(JIRInstruction::IntLessEqual, 3, 5, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntLessEqual, 5, 5, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntLessEqual, 7, 5, Some(0), 1, Some(1));

        // Greater than or equal
        test_binary_op(JIRInstruction::IntGreaterEqual, 5, 3, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntGreaterEqual, 5, 5, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::IntGreaterEqual, 3, 5, Some(0), 1, Some(1));
    }

    #[test]
    fn test_int_signed_comparison() {
        // In 8-bit two's complement: 0x80 = -128, 0xFF = -1

        // Signed less than
        test_binary_op(JIRInstruction::IntSLess, 0x80, 0x01, Some(1), 1, Some(1)); // -128 < 1
        test_binary_op(JIRInstruction::IntSLess, 0xFF, 0x01, Some(1), 1, Some(1)); // -1 < 1
        test_binary_op(JIRInstruction::IntSLess, 0x01, 0x80, Some(0), 1, Some(1)); // 1 < -128 = false

        // Signed greater than
        test_binary_op(JIRInstruction::IntSGreater, 0x01, 0x80, Some(1), 1, Some(1)); // 1 > -128
        test_binary_op(JIRInstruction::IntSGreater, 0x01, 0xFF, Some(1), 1, Some(1)); // 1 > -1
        test_binary_op(JIRInstruction::IntSGreater, 0x80, 0x01, Some(0), 1, Some(1)); // -128 > 1 = false

        // Signed less than or equal
        test_binary_op(JIRInstruction::IntSLessEqual, 0x80, 0x80, Some(1), 1, Some(1)); // -128 <= -128
        test_binary_op(JIRInstruction::IntSLessEqual, 0x80, 0xFF, Some(1), 1, Some(1)); // -128 <= -1
        test_binary_op(JIRInstruction::IntSLessEqual, 0x01, 0x80, Some(0), 1, Some(1)); // 1 <= -128 = false

        // Signed greater than or equal
        test_binary_op(JIRInstruction::IntSGreaterEqual, 0xFF, 0xFF, Some(1), 1, Some(1)); // -1 >= -1
        test_binary_op(JIRInstruction::IntSGreaterEqual, 0xFF, 0x80, Some(1), 1, Some(1)); // -1 >= -128
        test_binary_op(JIRInstruction::IntSGreaterEqual, 0x80, 0x7F, Some(0), 1, Some(1)); // -128 >= 127 = false
    }

    //==================================================
    // FLOATING POINT TESTS
    //==================================================

    #[test]
    fn test_float_arithmetic() {
        // Convert test values to bit representations
        let float_3_5 = f32::to_bits(3.5) as u128;
        let float_1_5 = f32::to_bits(1.5) as u128;
        let float_5_0 = f32::to_bits(5.0) as u128;
        let float_0_0 = f32::to_bits(0.0) as u128;

        // Float addition
        let result = f32::to_bits(3.5 + 1.5) as u128;
        test_binary_op(JIRInstruction::FloatAdd, float_3_5, float_1_5, Some(result), 4, None);

        // Float subtraction
        let result = f32::to_bits(3.5 - 1.5) as u128;
        test_binary_op(JIRInstruction::FloatSubtract, float_3_5, float_1_5, Some(result), 4, None);

        // Float multiplication
        let result = f32::to_bits(3.5 * 1.5) as u128;
        test_binary_op(JIRInstruction::FloatMul, float_3_5, float_1_5, Some(result), 4, None);

        // Float division
        let result = f32::to_bits(5.0 / 1.5) as u128;
        test_binary_op(JIRInstruction::FloatDiv, float_5_0, float_1_5, Some(result), 4, None);

        // Float division by zero should return None
        test_binary_op(JIRInstruction::FloatDiv, float_5_0, float_0_0, None, 4, None);

        // Test with 64-bit floats
        let double_2_5 = f64::to_bits(2.5) as u128;
        let double_1_5 = f64::to_bits(1.5) as u128;
        let result = f64::to_bits(2.5 + 1.5) as u128;
        test_binary_op(JIRInstruction::FloatAdd, double_2_5, double_1_5, Some(result), 8, None);
    }

    #[test]
    fn test_float_comparison() {
        // Convert test values to bit representations
        let float_3_5 = f32::to_bits(3.5) as u128;
        let float_1_5 = f32::to_bits(1.5) as u128;
        let float_3_5_dup = f32::to_bits(3.5) as u128; // Duplicate to test equality

        // Float equals
        test_binary_op(JIRInstruction::FloatEquals, float_3_5, float_3_5_dup, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatEquals, float_3_5, float_1_5, Some(0), 4, Some(1));

        // Float not equals
        test_binary_op(JIRInstruction::FloatNotEquals, float_3_5, float_1_5, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatNotEquals, float_3_5, float_3_5_dup, Some(0), 4, Some(1));

        // Float less than
        test_binary_op(JIRInstruction::FloatLess, float_1_5, float_3_5, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatLess, float_3_5, float_1_5, Some(0), 4, Some(1));

        // Float greater than
        test_binary_op(JIRInstruction::FloatGreater, float_3_5, float_1_5, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatGreater, float_1_5, float_3_5, Some(0), 4, Some(1));

        // Float less than or equal
        test_binary_op(JIRInstruction::FloatLessEqual, float_1_5, float_3_5, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatLessEqual, float_3_5, float_3_5_dup, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatLessEqual, float_3_5, float_1_5, Some(0), 4, Some(1));

        // Float greater than or equal
        test_binary_op(JIRInstruction::FloatGreaterEqual, float_3_5, float_1_5, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatGreaterEqual, float_3_5, float_3_5_dup, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatGreaterEqual, float_1_5, float_3_5, Some(0), 4, Some(1));
    }

    #[test]
    fn test_float_special_values() {
        // Special floating point values
        let float_nan = f32::to_bits(f32::NAN) as u128;
        let float_inf = f32::to_bits(f32::INFINITY) as u128;
        let float_neg_inf = f32::to_bits(f32::NEG_INFINITY) as u128;
        let float_0 = f32::to_bits(0.0) as u128;

        // NaN comparison tests
        test_binary_op(JIRInstruction::FloatEquals, float_nan, float_nan, Some(0), 4, Some(1));
        test_binary_op(JIRInstruction::FloatNotEquals, float_nan, float_nan, Some(1), 4, Some(1));

        // Infinity tests
        test_binary_op(JIRInstruction::FloatLess, float_0, float_inf, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatLess, float_neg_inf, float_0, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::FloatGreater, float_inf, float_0, Some(1), 4, Some(1));

        // Test NaN detection
        test_unary_op(JIRInstruction::Nan, float_nan, Some(1), 4, Some(1));
        test_unary_op(JIRInstruction::Nan, float_0, Some(0), 4, Some(1));

        // Test using the result of invalid operations
        let div_by_zero = f32::to_bits(1.0 / 0.0) as u128;
        test_unary_op(JIRInstruction::Nan, div_by_zero, Some(0), 4, Some(1)); // Infinity, not NaN

        let sqrt_neg = f32::to_bits(f32::NAN) as u128; // sqrt(-1) would be NaN
        test_unary_op(JIRInstruction::Nan, sqrt_neg, Some(1), 4, Some(1));
    }

    //==================================================
    // BOOLEAN OPERATIONS TESTS
    //==================================================

    #[test]
    fn test_boolean_operations() {
        // Boolean AND
        test_binary_op(JIRInstruction::BoolAnd, 1, 1, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolAnd, 1, 0, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::BoolAnd, 0, 1, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::BoolAnd, 0, 0, Some(0), 1, Some(1));

        // Any non-zero value should be treated as true
        test_binary_op(JIRInstruction::BoolAnd, 42, 1, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolAnd, 255, 255, Some(1), 1, Some(1));

        // Boolean OR
        test_binary_op(JIRInstruction::BoolOr, 1, 1, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolOr, 1, 0, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolOr, 0, 1, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolOr, 0, 0, Some(0), 1, Some(1));
        // Non-zero values
        test_binary_op(JIRInstruction::BoolOr, 42, 0, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolOr, 255, 255, Some(1), 1, Some(1));

        // Boolean XOR
        test_binary_op(JIRInstruction::BoolXor, 1, 1, Some(0), 1, Some(1));
        test_binary_op(JIRInstruction::BoolXor, 1, 0, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolXor, 0, 1, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolXor, 0, 0, Some(0), 1, Some(1));
        // Non-zero values
        test_binary_op(JIRInstruction::BoolXor, 42, 0, Some(1), 1, Some(1));
        test_binary_op(JIRInstruction::BoolXor, 42, 255, Some(0), 1, Some(1));

        // Test with different input sizes but same output size
        test_binary_op(JIRInstruction::BoolAnd, 1, 1, Some(1), 2, Some(1));
        test_binary_op(JIRInstruction::BoolOr, 1, 1, Some(1), 4, Some(1));
        test_binary_op(JIRInstruction::BoolXor, 1, 1, Some(0), 8, Some(1));
    }
}
