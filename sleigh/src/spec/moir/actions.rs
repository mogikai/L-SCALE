use crate::compiler::InternalCompilerResult;
use crate::spec::jir::context_stack::ContextStack;
use crate::spec::jir::local_resolver::JIRResolver;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRValue,
};
use serde_derive::{
    Deserialize,
    Serialize,
};

use super::field::FieldId;
use super::scope::MOIRLocalId;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    Integer(u128),
}

/// Disassembly actions are used for dynamic calculations at disassembly time to for example
/// calculate absolute addresses for relative branches based on the current instruction address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MOIRAction {
    local_id: MOIRLocalId,
    context_field: Option<FieldId>, // if this is Some, the context is modified
    expression: MOIRPatternExpression,
}

impl MOIRAction {
    pub(crate) fn new(
        local_id: MOIRLocalId,
        context: Option<FieldId>,
        expression: MOIRPatternExpression,
    ) -> Self {
        Self {
            local_id,
            context_field: context,
            expression,
        }
    }

    pub(crate) fn evaluate(
        &self,
        resolver: &mut JIRResolver,
        context_provider: &mut ContextStack,
    ) -> InternalCompilerResult {
        let value = self.expression.evaluate(resolver)?;

        trace_jir!("Action is setting local {} := {}", self.local_id, value);
        resolver.define_id(self.local_id, JIRValue::Const(JIRConst::new(value, 0)));

        if let Some(context_field) = self.context_field {
            trace_jir!("Action is setting context field {} := {}", context_field, value);
            context_provider.set_field_value(context_field, value)?;
        }

        Ok(())
    }

    pub(crate) fn modifies_context(&self) -> bool {
        self.context_field.is_some()
    }
}

impl MOIRPatternExpression {
    // TODO should this be signed?
    pub(crate) fn evaluate(
        &self,
        resolver: &JIRResolver,
    ) -> InternalCompilerResult<u128> {
        Ok(match self {
            MOIRPatternExpression::Add(lhs, rhs) => lhs
                .evaluate(resolver)?
                .wrapping_add(rhs.evaluate(resolver)?),
            MOIRPatternExpression::Sub(lhs, rhs) => lhs.evaluate(resolver)? - rhs.evaluate(resolver)?,
            MOIRPatternExpression::Mul(lhs, rhs) => lhs.evaluate(resolver)? * rhs.evaluate(resolver)?,
            MOIRPatternExpression::Div(lhs, rhs) => lhs.evaluate(resolver)? / rhs.evaluate(resolver)?,
            MOIRPatternExpression::Shl(lhs, rhs) => lhs.evaluate(resolver)? << rhs.evaluate(resolver)?,
            MOIRPatternExpression::Shr(lhs, rhs) => {
                ((lhs.evaluate(resolver)? as i128) >> rhs.evaluate(resolver)?) as u128
            }
            MOIRPatternExpression::And(lhs, rhs) => lhs.evaluate(resolver)? & rhs.evaluate(resolver)?,
            MOIRPatternExpression::Or(lhs, rhs) => lhs.evaluate(resolver)? | rhs.evaluate(resolver)?,
            MOIRPatternExpression::Xor(lhs, rhs) => lhs.evaluate(resolver)? ^ rhs.evaluate(resolver)?,
            MOIRPatternExpression::Not(expr) => !expr.evaluate(resolver)?,
            MOIRPatternExpression::Local(local_id) => resolver.resolve_local_id_to_const(*local_id)?,
            MOIRPatternExpression::Integer(value) => *value,
        })
    }
}
