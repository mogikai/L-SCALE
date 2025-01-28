use crate::compiler::InternalCompilerResult;
use crate::spec::jir::local_resolver::JIRResolver;

use super::field::FieldId;
use super::scope::MOIRLocalId;

#[derive(Debug, Clone)]
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
    Local(MOIRLocalId),
    Integer(u64),
}

/// We differentiate between actions that change context and actions that set
/// a temporary.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MOIRActionTarget {
    ContextField(FieldId),
    Local(MOIRLocalId),
}

/// Disassembly actions are used for dynamic calculations at disassembly time to for example
/// calculate absolute addresses for relative branches based on the current instruction address.
#[derive(Debug, Clone)]
pub(crate) struct MOIRAction {
    pub(crate) lhs: MOIRActionTarget,
    expression: MOIRPatternExpression,
}

impl MOIRAction {
    pub(crate) fn new(
        lhs: MOIRActionTarget,
        expression: MOIRPatternExpression,
    ) -> Self {
        Self { lhs, expression }
    }
}

impl MOIRAction {
    pub(crate) fn evaluate(
        &self,
        resolver: &JIRResolver,
    ) -> InternalCompilerResult<u64> {
        self.expression.evaluate(resolver)
    }
}

impl MOIRPatternExpression {
    // TODO should this be signed?
    pub(crate) fn evaluate(
        &self,
        resolver: &JIRResolver,
    ) -> InternalCompilerResult<u64> {
        Ok(match self {
            MOIRPatternExpression::Add(lhs, rhs) => lhs.evaluate(resolver)? + rhs.evaluate(resolver)?,
            MOIRPatternExpression::Sub(lhs, rhs) => lhs.evaluate(resolver)? - rhs.evaluate(resolver)?,
            MOIRPatternExpression::Mul(lhs, rhs) => lhs.evaluate(resolver)? * rhs.evaluate(resolver)?,
            MOIRPatternExpression::Div(lhs, rhs) => lhs.evaluate(resolver)? / rhs.evaluate(resolver)?,
            MOIRPatternExpression::Shl(lhs, rhs) => lhs.evaluate(resolver)? << rhs.evaluate(resolver)?,
            MOIRPatternExpression::Shr(lhs, rhs) => {
                ((lhs.evaluate(resolver)? as i64) >> rhs.evaluate(resolver)?) as u64
            }
            MOIRPatternExpression::And(lhs, rhs) => lhs.evaluate(resolver)? & rhs.evaluate(resolver)?,
            MOIRPatternExpression::Or(lhs, rhs) => lhs.evaluate(resolver)? | rhs.evaluate(resolver)?,
            MOIRPatternExpression::Xor(lhs, rhs) => lhs.evaluate(resolver)? ^ rhs.evaluate(resolver)?,
            MOIRPatternExpression::Not(expr) => !expr.evaluate(resolver)?,
            MOIRPatternExpression::Local(local_id) => resolver
                .resolve_local_id_to_const(*local_id)?
                .try_into()
                .expect("Actions only support 64-bit values"),
            MOIRPatternExpression::Integer(value) => *value,
        })
    }
}
