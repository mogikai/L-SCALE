use std::collections::HashMap;

use crate::compiler::SleighCompilerResult;
use crate::compiler::ast::Spanned;
use crate::compiler::error::SleighCompilerError;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRValue,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
};

use super::common::MOIRSize;
use super::context::MOIRContextId;
use super::field::{
    FieldId,
    MOIRBitRange,
};
use super::scope::{
    MOIRLocalId,
    MOIRTemporaryId,
};
use super::varnode::{
    MOIRLocal,
    MOIRReference,
};

/// Actions can use fields (either from context or the bit pattern) as well as
/// previously defined locals (i.e. temporaries) as parameters.
#[derive(Debug, Eq, PartialEq, Hash)]
pub(crate) enum MOIRActionLocal {
    Field(FieldId),
    Temporary(MOIRTemporaryId),
}

#[derive(Debug)]
pub(crate) enum MOIRPatternExpression {
    Add(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Sub(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Mul(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Div(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Shl(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Shr(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>), // arithmetic right-shift
    And(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Or(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Xor(Box<MOIRPatternExpression>, Box<MOIRPatternExpression>),
    Not(Box<MOIRPatternExpression>),
    Local(MOIRActionLocal),
    Integer(u64), // TODO signed? unsigned? 64 or 128 bit?
}

/// We differentiate between actions that change context and actions that set
/// a temporary.
#[derive(Debug)]
pub(crate) enum MOIRActionTarget {
    ContextField(FieldId),
    Temporary(MOIRTemporaryId),
}

/// Disassembly actions are used for dynamic calculations at disassembly time to for example
/// calculate absolute addresses for relative branches based on the current instruction address.
#[derive(Debug)]
pub(crate) struct MOIRAction {
    pub(crate) lhs: MOIRActionTarget,
    expression: MOIRPatternExpression,
}

impl MOIRAction {
    pub(crate) fn new(
        lhs: MOIRActionTarget,
        expression: MOIRPatternExpression,
    ) -> Self {
        Self {
            lhs,
            expression,
        }
    }
}

impl MOIRAction {
    pub(crate) fn evaluate(
        &self,
        values: &HashMap<MOIRActionLocal, u64>,
    ) -> SleighCompilerResult<u64> {
        self.expression.evaluate(values)
    }
}

impl MOIRPatternExpression {
    // TODO should this be signed?
    pub(crate) fn evaluate(
        &self,
        values: &HashMap<MOIRActionLocal, u64>,
    ) -> SleighCompilerResult<u64> {
        Ok(match self {
            MOIRPatternExpression::Add(lhs, rhs) => lhs.evaluate(values)? + rhs.evaluate(values)?,
            MOIRPatternExpression::Sub(lhs, rhs) => lhs.evaluate(values)? - rhs.evaluate(values)?,
            MOIRPatternExpression::Mul(lhs, rhs) => lhs.evaluate(values)? * rhs.evaluate(values)?,
            MOIRPatternExpression::Div(lhs, rhs) => lhs.evaluate(values)? / rhs.evaluate(values)?,
            MOIRPatternExpression::Shl(lhs, rhs) => lhs.evaluate(values)? << rhs.evaluate(values)?,
            MOIRPatternExpression::Shr(lhs, rhs) => lhs.evaluate(values)? >> rhs.evaluate(values)?,
            MOIRPatternExpression::And(lhs, rhs) => lhs.evaluate(values)? & rhs.evaluate(values)?,
            MOIRPatternExpression::Or(lhs, rhs) => lhs.evaluate(values)? | rhs.evaluate(values)?,
            MOIRPatternExpression::Xor(lhs, rhs) => lhs.evaluate(values)? ^ rhs.evaluate(values)?,
            MOIRPatternExpression::Not(expr) => !expr.evaluate(values)?,
            MOIRPatternExpression::Local(local) => *values.get(local).ok_or_else(|| -> SleighCompilerError {
                todo!("Generate error report for missing local {:?} value in pattern expression", local)
            })?,
            MOIRPatternExpression::Integer(value) => u64::from(*value),
        })
    }
}
