use crate::spec::builder::StringId;
use crate::spec::jir::instruction::TargetInstruction;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
};
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::scope::MOIRLocalId;
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRReference,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecificationError,
};
use mogi_utilities::addresses::EVAddr;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug)]
pub struct JIRResolver<'a> {
    constructor: &'a MOIRConstructor,
    values: HashMap<MOIRLocalId, JIRValue>,
    subtables: HashMap<TableId, JIRResolver<'a>>,
    inst_start: EVAddr,
    inst_next: EVAddr,
    /// Since inst_next2 is rarely needed it can be set to None
    inst_next2: Option<EVAddr>,
}

impl<'a> JIRResolver<'a> {
    pub fn new(
        constructor: &'a MOIRConstructor,
        inst_start: EVAddr,
        inst_next: EVAddr,
        inst_next2: Option<EVAddr>,
    ) -> Self {
        Self {
            constructor,
            values: Default::default(),
            subtables: Default::default(),
            inst_start,
            inst_next,
            inst_next2,
        }
    }

    pub fn constructor(&self) -> &MOIRConstructor {
        self.constructor
    }

    pub fn define_ident(
        &mut self,
        ident: StringId,
        value: JIRValue,
    ) -> ProcessorSpecResult {
        let local = self
            .constructor
            .local_scope
            .local(ident)
            .ok_or_else(|| todo!("Generate Error"))?;
        if local.refers_to_varnode() {
            todo!("Generate error: Overriding varnodes is disallowed during local resolving")
        }
        // TODO: Write code that checks the size constraints of the value against the defined slice
        let local_id = self
            .constructor
            .local_scope
            .local_id(ident)
            .ok_or_else(|| todo!("Generate error"))?;
        let previous = self.values.insert(local_id, value);
        assert!(previous.is_none());
        Ok(())
    }

    pub fn define_subtable_resolver(
        &mut self,
        subtable: TableId,
        resolver: JIRResolver<'a>,
    ) -> ProcessorSpecResult {
        self.subtables.insert(subtable, resolver);
        Ok(())
    }

    pub fn subtable_resolver(
        &self,
        subtable: TableId,
    ) -> ProcessorSpecResult<&JIRResolver<'a>> {
        self.subtables
            .get(&subtable)
            .ok_or(ProcessorSpecificationError::UnresolvedSubtable)
    }

    pub fn resolve_local(
        &self,
        local: MOIRLocal,
    ) -> ProcessorSpecResult<JIRValue> {
        match local {
            MOIRLocal::Local(id) => self.resolve_local_id(id),
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
        local: MOIRLocalId,
    ) -> ProcessorSpecResult<JIRValue> {
        let value = self
            .constructor
            .local_scope
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
                    debug!("Constructor: {:?}", self.constructor);
                    debug!("Scope is {}", self.constructor.local_scope);
                    debug!("Local id is {local}");
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

    pub fn inst_start(&self) -> EVAddr {
        self.inst_start
    }

    pub fn inst_next(&self) -> EVAddr {
        self.inst_next
    }
}
