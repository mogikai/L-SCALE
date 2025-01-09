use crate::compiler::error::SleighCompilerError;
use crate::jinst;
use crate::spec::builder::StringId;
use crate::spec::jir::common::JIRSize;
use crate::spec::jir::instruction::{
    TargetInstruction,
    TargetInstructionBuilder,
};
use crate::spec::jir::space::JIRSpaceType;
use crate::spec::jir::trace_jir;
use crate::spec::jir::values::{
    JIRConst,
    JIRSlice,
    JIRValue,
    JIRVarNodeId,
};
use crate::spec::moir::address_space::SpaceId;
use crate::spec::moir::constructor::MOIRConstructor;
use crate::spec::moir::scope::{
    MOIRLocalId,
    MOIRMaybeSize,
};
use crate::spec::moir::table::{
    MOIRExport,
    TableId,
};
use crate::spec::moir::varnode::{
    MOIRConst,
    MOIRLocal,
    MOIRReference,
};
use crate::spec::{
    ProcessorSpecResult,
    ProcessorSpecification,
    ProcessorSpecificationError,
};
use mogi_utilities::addresses::EVAddr;
use std::collections::HashMap;
use tracing::debug;

pub enum JIRResolvedValue {
    Value(JIRValue),
    Reference {
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    },
}

impl From<JIRConst> for JIRResolvedValue {
    fn from(value: JIRConst) -> Self {
        JIRResolvedValue::Value(value.into())
    }
}

impl From<JIRSlice> for JIRResolvedValue {
    fn from(value: JIRSlice) -> Self {
        JIRResolvedValue::Value(value.into())
    }
}

impl From<JIRValue> for JIRResolvedValue {
    fn from(value: JIRValue) -> Self {
        JIRResolvedValue::Value(value)
    }
}

#[derive(Debug)]
pub struct JIRResolver<'a> {
    spec: &'a ProcessorSpecification,
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
        spec: &'a ProcessorSpecification,
        constructor: &'a MOIRConstructor,
        inst_start: EVAddr,
        inst_next: EVAddr,
        inst_next2: Option<EVAddr>,
    ) -> Self {
        Self {
            spec,
            constructor,
            values: Default::default(),
            subtables: Default::default(),
            inst_start,
            inst_next,
            inst_next2,
        }
    }

    // TODO remove
    pub(crate) fn print_hierarchy(&self, depth: u32) {
        for _ in 0..depth {
            eprint!(" ");
        }

        eprintln!("|-> {:?}", self.constructor.display);
        for subtable_resolver in self.subtables.values() {
            subtable_resolver.print_hierarchy(depth + 4);
        }
    }

    /// Resolve the unresolved temporaries for the associated constructor.
    /// This will allocate temp varnodes in the [TargetInstruction] for each temporary
    /// in the scope.
    pub fn realize_temporaries(
        &mut self,
        instruction_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult {
        for (local, temp) in self.constructor.local_scope.iter_used_temps() {
            let MOIRMaybeSize::Determined(size) = temp.size else {
                todo!("Generate error for undetermined temporary size")
            };
            let temp_varnode = instruction_builder.alloc_temporary(size)?;
            self.values
                .insert(local, JIRValue::Slice(JIRSlice::new_temp(temp_varnode, 0, size)));
        }
        Ok(())
    }

    pub fn temp_space(&self) -> SpaceId {
        self.spec.temp_space
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

    pub fn build_subtable(
        &mut self,
        instruction_builder: &mut TargetInstructionBuilder,
        subtable: TableId,
    ) -> ProcessorSpecResult {
        trace_jir!("Building subtable {subtable}");
        let resolver = self
            .subtables
            .get_mut(&subtable)
            .ok_or(ProcessorSpecificationError::UnresolvedSubtable)?;
        trace_jir!("{}", resolver.constructor.mcode_debug_string(&self.spec.interner));
        resolver
            .constructor
            .generate_jcode_subtable(resolver, instruction_builder)
    }

    pub fn resolve_local(
        &self,
        instruction_builder: &mut TargetInstructionBuilder,
        local: MOIRLocal,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        match local {
            MOIRLocal::Local(id) => self.resolve_local_id(instruction_builder, id),
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
        instruction_builder: &mut TargetInstructionBuilder,
        local: MOIRLocalId,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        let value = self
            .constructor
            .local_scope
            .resolve_local(local)
            .map_err(|_| todo!("Map error correctly"))?;
        trace_jir!("Local {local} is {value:?}");
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
            MOIRReference::Temporary(_) => match self.values.get(&local).copied() {
                None => todo!("Generate error for unresolved temporary."),
                Some(value) => value.into(),
            },
            MOIRReference::Field(_) => {
                let Some(value) = self.values.get(&local).copied() else {
                    debug!("Constructor: {:?}", self.constructor);
                    debug!("Scope is {}", self.constructor.local_scope);
                    debug!("Local id is {local}");
                    todo!("Generate error for a local that cannot be resolved because it was not defined")
                };
                value.into()
            }
            MOIRReference::SubTable(table_id) => {
                let subtable_resolver = self
                    .subtables
                    .get(&table_id)
                    .ok_or_else(|| todo!("Generate error: 'Failed to match subtable'"))?;
                // Check the subtable resolver for the symbol that is exported and resolve it to a JIRValue.
                debug!("Descending into subtable {table_id} to resolve export");
                subtable_resolver.resolve_export(instruction_builder)?
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

    pub fn resolve_export(
        &self,
        instruction_builder: &mut TargetInstructionBuilder,
    ) -> ProcessorSpecResult<JIRResolvedValue> {
        let Some(export) = self.constructor.export.as_ref() else {
            todo!("generate error for missing export")
        };
        // If the export is a local, we simply return the resolved local
        // This might mean descending recursively into export resolving.
        match export {
            MOIRExport::Local(local) => self.resolve_local(instruction_builder, *local),
            MOIRExport::MemReference { address, space, size } => {
                // We need to recursively resolve the address, since it might also be a dynamic reference
                let address = self.resolve_local(instruction_builder, *address)?;
                let address = match address {
                    JIRResolvedValue::Value(address) => address,
                    JIRResolvedValue::Reference { address, space, size } => {
                        // If this is a load from the constant space, we can just interpret the address
                        // offset as the constant value. I.e. the position in the const address space
                        // corresponds to its value.
                        // TODO: Replace with api call
                        if let Some(optimized) = self.optimize_const_load(address, space, size)? {
                            return Ok(optimized.into());
                        }

                        // In case of a dynamic reference as address for the current reference (i.e. **addr)
                        // we need to insert a load instruction to resolve the first indirection to *addr
                        // The final load or store operation needs to be inserted by the instruction resolver,
                        // since only it knows whether we need a load or store instruction.
                        let tmp = JIRSlice::new_temp(instruction_builder.alloc_temporary(size)?, 0, size);
                        instruction_builder.push_jir_instruction(jinst!(tmp = *[space]address))?;
                        // Create an rvalue
                        JIRValue::Slice(tmp)
                    }
                };
                Ok(JIRResolvedValue::Reference {
                    address,
                    space: *space,
                    size: size.ok_or_else(|| todo!("Generate error for missing export size"))?,
                })
            }
        }
    }

    /// Interpret the provided address as a constant. I.e. the address' offset is equal to the const's value
    pub fn optimize_const_load(
        &self,
        address: JIRValue,
        space: SpaceId,
        size: JIRSize,
    ) -> ProcessorSpecResult<Option<JIRConst>> {
        let JIRSpaceType::Const = self
            .spec
            .symbol_tables
            .spaces
            .get(space)
            .expect("This is expected")
            .r#type
        else {
            // Space is not const. Nothing to optimize.
            return Ok(None);
        };
        match address {
            JIRValue::Slice(slice) => {
                if let JIRVarNodeId::VarNode(id) = slice.varnode_id {
                    // Interpret the offset + start as the constant value.
                    let varnode = self.spec.varnode_by_id(id)?;
                    Ok(Some(JIRConst::new((varnode.offset + u64::from(slice.start)).into(), size)))
                } else {
                    todo!("This should not happen.")
                }
            }
            JIRValue::Const(c) => Ok(Some(JIRConst::new(c.value(), size))),
        }
    }

    pub fn inst_start(&self) -> EVAddr {
        self.inst_start
    }

    pub fn inst_next(&self) -> EVAddr {
        self.inst_next
    }
}
