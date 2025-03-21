use crate::compiler::InternalCompilerResult;
use crate::spec::jir::common::JIRSize;
use crate::spec::moir::common::MOIRSize;

use self::jumplist::JumplistBuilder;
use crate::spec::builder::{
    MOIRSymbolId,
    MOIRSymbolTables,
    StringId,
};
use crate::spec::jir::custom_op::JIRCustomOp;
use crate::spec::jir::jumplist::{
    Jumplist,
    JumplistId,
};
use crate::spec::jir::space::{
    JIRSpace,
    JIRSpaceType,
};
use crate::spec::jir::values::{
    JIRVarNode,
    VarnodeGroup,
    VarnodeGroupId,
};
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceType,
    SpaceId,
};
use crate::spec::moir::context::{
    ContextId,
    MOIRContext,
};
use crate::spec::moir::custom_op::MOIRCustomOp;
use crate::spec::moir::table::TableId;
use crate::spec::moir::varnode::{
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::{
    CustomOpId,
    Ident,
    JIRSymbolId,
    JIRSymbolTables,
};
use itertools::Itertools;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};
use std::collections::HashMap;

mod jumplist;
use mogi_sleigh_parser::{
    ParserContext,
    SleighInterner,
    Spanned,
};
use tracing::{
    debug,
    warn,
};

#[derive(Debug)]
pub(super) struct MOIRToJIRTransformer<'a> {
    moir: MOIRData<'a>,
    jir: JIRData,
}

#[derive(Debug)]
pub struct JIRData {
    pub symbol_ids: PrimaryMap<Ident, JIRSymbolId>,
    pub symbol_strings: HashMap<Ident, StringId>,
    pub symbol_tables: JIRSymbolTables,
    pub jumplist_cache: PrimaryMap<JumplistId, Jumplist>,
    pub string_to_ident_mapping: HashMap<StringId, Ident>,
    pub varnode_groups: PrimaryMap<VarnodeGroupId, VarnodeGroup>,
    pub varnode_to_group: SecondaryMap<VarNodeId, Option<VarnodeGroupId>>,
}

#[derive(Debug)]
struct MOIRData<'a> {
    interner: &'a SleighInterner,
    symbol_ids: PrimaryMap<Ident, MOIRSymbolId>,
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    string_to_ident_mapping: HashMap<StringId, Ident>,
    context: &'a PrimaryMap<ContextId, MOIRContext>,
    root_table_id: TableId,
}

impl<'a> MOIRToJIRTransformer<'a> {
    pub fn new(
        interner: &'a SleighInterner,
        symbol_ids: PrimaryMap<Ident, MOIRSymbolId>,
        symbol_strings: HashMap<Ident, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        string_to_ident_mapping: HashMap<StringId, Ident>,
        context: &'a PrimaryMap<ContextId, MOIRContext>,
        root_table_id: TableId,
    ) -> Self {
        Self {
            moir: MOIRData {
                interner,
                symbol_ids,
                symbol_strings,
                symbol_tables,
                string_to_ident_mapping,
                context,
                root_table_id,
            },
            jir: JIRData {
                symbol_ids: Default::default(),
                symbol_strings: Default::default(),
                symbol_tables: Default::default(),
                jumplist_cache: Default::default(),
                string_to_ident_mapping: Default::default(),
                varnode_groups: Default::default(),
                varnode_to_group: Default::default(),
            },
        }
    }

    pub fn transform(mut self) -> InternalCompilerResult<JIRData> {
        self.jir.symbol_ids = self
            .moir
            .symbol_ids
            .into_iter()
            .map(|(ident, symbol_id)| {
                let symbol_id = match symbol_id {
                    MOIRSymbolId::Undefined => todo!("Generate error for undefined symbol"),
                    MOIRSymbolId::Macro(id) => JIRSymbolId::Macro(*id),
                    MOIRSymbolId::Space(id) => JIRSymbolId::Space(*id),
                    MOIRSymbolId::Table(id) => JIRSymbolId::Table(*id),
                    MOIRSymbolId::Token(id) => JIRSymbolId::Token(*id),
                    MOIRSymbolId::Field(id) => JIRSymbolId::Field(*id),
                    MOIRSymbolId::VarNode(id) => JIRSymbolId::VarNode(*id),
                    MOIRSymbolId::BitrangeRegister(id) => JIRSymbolId::BitrangeRegister(*id),
                    MOIRSymbolId::CustomOp(id) => JIRSymbolId::CustomOp(*id),
                };
                (ident, symbol_id)
            })
            .collect();
        self.jir.symbol_strings = self
            .moir
            .symbol_strings
            .into_iter()
            .map(|(k, v)| (k, v.into_value()))
            .collect();

        let jumplist_cache = JumplistBuilder::new(
            &self.moir.symbol_tables.tables,
            &self.moir.symbol_tables.fields,
            &self.moir.symbol_tables.tokens,
            &self.moir.context,
            &self.moir.symbol_tables.varnodes,
            &self.moir.symbol_tables.constructors,
        )
        .transform(self.moir.root_table_id)?;

        self.jir.jumplist_cache = jumplist_cache;
        let (varnodes, varnode_groups, varnode_to_group) = transform_varnodes(self.moir.symbol_tables.varnodes, 8)?;
        self.jir.varnode_groups = varnode_groups;
        self.jir.varnode_to_group = varnode_to_group;
        self.jir.symbol_tables = JIRSymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            constructors: self.moir.symbol_tables.constructors,
            varnodes,
            custom_ops: transform_custom_ops(self.moir.interner, self.moir.symbol_tables.custom_ops)?,
            fields: self.moir.symbol_tables.fields,
        };

        self.jir.string_to_ident_mapping = self.moir.string_to_ident_mapping;
        Ok(self.jir)
    }
}

fn transform_custom_ops(
    interner: &SleighInterner,
    custom_ops: PrimaryMap<CustomOpId, MOIRCustomOp>,
) -> InternalCompilerResult<PrimaryMap<CustomOpId, Option<JIRCustomOp>>> {
    Ok(custom_ops
        .into_iter()
        .map(|(id, op)| match op {
            MOIRCustomOp::Declared { name } => {
                let name = interner
                    .string(*name)
                    .expect("The string has to exist since we saved an id to it.");
                // TODO: Emit a compiler warning instead of a log output
                warn!("Custom op '{name}' is declared but not defined. Ignoring.");
                (id, None)
            }
            MOIRCustomOp::Defined { name, .. } => (
                id,
                Some(JIRCustomOp {
                    name: interner
                        .string(*name)
                        .expect("The string has to exist since we saved an id to it.")
                        .to_string(),
                }),
            ),
        })
        .collect::<PrimaryMap<CustomOpId, Option<JIRCustomOp>>>())
}

fn transform_spaces(spaces: PrimaryMap<SpaceId, MOIRSpace>) -> InternalCompilerResult<PrimaryMap<SpaceId, JIRSpace>> {
    Ok(spaces
        .into_iter()
        .map(|(ident, space)| {
            let r#type = match space.r#type {
                MOIRSpaceType::Ram => JIRSpaceType::Ram,
                MOIRSpaceType::Rom => JIRSpaceType::Rom,
                MOIRSpaceType::Register => JIRSpaceType::Register,
                MOIRSpaceType::Const => JIRSpaceType::Const,
            };
            let space = JIRSpace {
                r#type,
                address_size: space
                    .address_size
                    .try_into()
                    .expect("The address size must fit into JIRSize"),
                wordsize: space
                    .wordsize
                    .try_into()
                    .expect("The word size must fit into JIRSize"),
            };
            (ident, space)
        })
        .collect())
}

fn build_varnode_groups(
    varnodes: &PrimaryMap<VarNodeId, MOIRVarNode>
) -> (PrimaryMap<VarnodeGroupId, VarnodeGroup>, SecondaryMap<VarNodeId, Option<VarnodeGroupId>>) {
    let mut groups: PrimaryMap<VarnodeGroupId, VarnodeGroup> = Default::default();
    let mut to_group: SecondaryMap<VarNodeId, Option<VarnodeGroupId>> = Default::default();
    'next_varnode: for (ident, varnode) in varnodes.items() {
        // check if varnode intersects with any existing intersection group and if so,
        // insert into groupand update group offset and size
        for group_id in groups.keys() {
            let group = groups.get_mut(group_id).expect("This element must exist");

            let end_group = group.offset + u64::from(group.size);
            let end_varnode = varnode.offset + u64::from(varnode.size);
            if end_group > varnode.offset && end_varnode > group.offset {
                if varnode.offset < group.offset {
                    // V < G => move group start and update size
                    group.size += JIRSize::try_from(group.offset - varnode.offset).unwrap();
                    group.offset = varnode.offset;
                }

                group.size = group.size.max(varnode.size.try_into().unwrap());
                group.varnodes.push(ident);
                to_group[ident] = Some(group_id);
                continue 'next_varnode;
            }
        }

        // nothing found => create new group
        let new_group = VarnodeGroup::new(vec![ident], varnode.offset, varnode.size.try_into().unwrap());
        let new_group_id = groups.insert(new_group);
        to_group[ident] = Some(new_group_id);
    }

    (groups, to_group)
}

fn transform_varnodes(
    mut varnodes: PrimaryMap<VarNodeId, MOIRVarNode>,
    alignment: MOIRSize,
) -> InternalCompilerResult<(
    PrimaryMap<VarNodeId, JIRVarNode>,
    PrimaryMap<VarnodeGroupId, VarnodeGroup>,
    SecondaryMap<VarNodeId, Option<VarnodeGroupId>>,
)> {
    assert_ne!(alignment, 0);

    let mut prev_end = 0;
    for varnode in varnodes.values() {
        let end = varnode.offset + u64::from(varnode.size);
        if end > prev_end {
            prev_end = end;
        }
    }

    debug!("Varnode prev size = {:#?}", prev_end);

    // we compact the groups, but offsets inside each group must stay the same
    let (mut groups, to_group) = build_varnode_groups(&varnodes);
    let mut running_offset = 0;
    for group in groups.values_mut() {
        // align up
        running_offset = u64::try_from((running_offset as i64 + alignment as i64 - 1) & !(alignment as i64 - 1))
            .expect("If alignment is not zero this cannot underflow");

        // modify group and varnode offsets
        let offset_delta = group.offset - running_offset;
        group.offset = running_offset;
        for varnode_id in group.varnodes.iter() {
            let varnode = varnodes
                .get_mut(*varnode_id)
                .expect("This value must exist");
            varnode.offset -= offset_delta;
        }

        running_offset += u64::from(group.size);
    }

    debug!("Group compacting yielded size of {:#?}", running_offset);

    Ok((
        varnodes
            .into_iter()
            .map(|(ident, varnode)| {
                let varnode = JIRVarNode {
                    space_id: varnode.space,
                    offset: varnode.offset,
                    size: varnode
                        .size
                        .try_into()
                        .expect("The size must fit into JIRSize"),
                };
                (ident, varnode)
            })
            .collect(),
        groups,
        to_group,
    ))
}
