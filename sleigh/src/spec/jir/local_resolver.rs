use crate::spec::ProcessorSpecResult;
use crate::spec::builder::StringId;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRScope,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRReference,
};
use mogi_utilities::addresses::EVAddr;
use std::collections::HashMap;

#[derive(Debug)]
pub struct JIRLocalResolver {
    values: HashMap<MOIRLocalId, JIRValue>,
    inst_start: EVAddr,
    inst_next: EVAddr,
    /// Since inst_next2 is rarely needed it can be set to None
    inst_next2: Option<EVAddr>,
}

impl JIRLocalResolver {
    pub fn new(
        inst_start: EVAddr,
        inst_next: EVAddr,
        inst_next2: Option<EVAddr>,
    ) -> Self {
        Self {
            values: Default::default(),
            inst_start,
            inst_next,
            inst_next2,
        }
    }
}

impl JIRLocalResolver {
    pub fn define_ident(
        &mut self,
        scope: &MOIRScope,
        ident: StringId,
        value: JIRValue,
    ) -> ProcessorSpecResult {
        let local = scope.local(ident).ok_or_else(|| todo!("Generate Error"))?;
        if local.refers_to_varnode() {
            todo!("Generate error: Overriding varnodes is disallowed during local resolving")
        }
        // TODO: Write code that checks the size constraints of the value against the defined slice
        let local_id = scope
            .local_id(ident)
            .ok_or_else(|| todo!("Generate error"))?;
        self.values.insert(local_id, value);
        Ok(())
    }

    pub fn resolve_local(
        &self,
        scope: &MOIRScope,
        local: MOIRLocal,
    ) -> ProcessorSpecResult<JIRValue> {
        match local {
            MOIRLocal::Local(id) => self.resolve_local_id(scope, id),
            MOIRLocal::Const(value) => {
                let MOIRConst::Determined(value, size) = value else {
                    todo!("Generate error for undetermined size")
                };
                Ok(JIRConst::new(value, size).into())
            }
        }
    }

    pub fn resolve_local_id(
        &self,
        scope: &MOIRScope,
        local: MOIRLocalId,
    ) -> ProcessorSpecResult<JIRValue> {
        let value = scope
            .resolve_local(local)
            .map_err(|_| todo!("Map error correctly"))?;
        let start = value.start;
        let end = value
            .end
            .ok_or_else(|| todo!("Generate error for undefined size"))?;
        Ok(match value.reference {
            MOIRReference::VarNode(varnode_id) => {
                if self.values.contains_key(&local) {
                    todo!("Generate error for overridden varnode.")
                }
                JIRSlice::new(varnode_id, start, end).into()
            }
            MOIRReference::Temporary(_) => match self.values.get(&local) {
                None => todo!("Generate error for unresolved temporary."),
                Some(value) => *value,
            },
            MOIRReference::Field(_) | MOIRReference::SubTable(_) => {
                let Some(value) = self.values.get(&local).copied() else {
                    todo!("Generate error for a local that cannot be resolved because it was not defined")
                };
                value
            }
            MOIRReference::InstStart => JIRConst::new(self.inst_start.into(), 8).into(),
            MOIRReference::InstNext => JIRConst::new(self.inst_next.into(), 8).into(),
            MOIRReference::InstNext2 => {
                let Some(inst_next2) = self.inst_next2 else {
                    todo!("Generate error for inst_next2 undefined")
                };
                JIRConst::new(inst_next2.into(), 8).into()
            }
        })
    }
}
