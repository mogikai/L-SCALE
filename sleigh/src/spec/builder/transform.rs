use crate::compiler::InternalCompilerResult;
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
    GroupId,
    JIRGroup,
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
    JIRSymbolId,
    JIRSymbolTables,
};
use addr_align::Align;
use fnv::FnvHashMap;
use mogi_utilities::index_map::{
    PrimaryMap,
    SecondaryMap,
};

mod jumplist;
use crate::compiler::build::trace_build;
use mogi_sleigh_parser::{
    ParserContext,
    SleighInterner,
    Spanned,
};
use mogi_utilities::overlap::overlaps_generic;
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
    pub symbol_ids: FnvHashMap<StringId, JIRSymbolId>,
    pub symbol_strings: FnvHashMap<JIRSymbolId, Spanned<StringId>>,
    pub symbol_tables: JIRSymbolTables,
    pub jumplist_cache: PrimaryMap<JumplistId, Jumplist>,
    pub varnode_groups: PrimaryMap<VarnodeGroupId, VarnodeGroup>,
    pub varnode_to_group: SecondaryMap<VarNodeId, Option<GroupId>>,
    pub jir_groups: PrimaryMap<GroupId, JIRGroup>,
}

#[derive(Debug)]
struct MOIRData<'a> {
    interner: &'a SleighInterner,
    symbol_ids: FnvHashMap<StringId, MOIRSymbolId>,
    symbol_strings: FnvHashMap<MOIRSymbolId, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    context: &'a PrimaryMap<ContextId, MOIRContext>,
    root_table_id: TableId,
}

impl<'a> MOIRToJIRTransformer<'a> {
    pub fn new(
        interner: &'a SleighInterner,
        symbol_ids: FnvHashMap<StringId, MOIRSymbolId>,
        symbol_strings: FnvHashMap<MOIRSymbolId, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        context: &'a PrimaryMap<ContextId, MOIRContext>,
        root_table_id: TableId,
    ) -> Self {
        Self {
            moir: MOIRData {
                interner,
                symbol_ids,
                symbol_strings,
                symbol_tables,
                context,
                root_table_id,
            },
            jir: JIRData {
                symbol_ids: Default::default(),
                symbol_strings: Default::default(),
                symbol_tables: Default::default(),
                jumplist_cache: Default::default(),
                varnode_groups: Default::default(),
                varnode_to_group: Default::default(),
                jir_groups: Default::default(),
            },
        }
    }

    pub fn transform(mut self) -> InternalCompilerResult<JIRData> {
        self.jir.symbol_ids = self
            .moir
            .symbol_ids
            .into_iter()
            .map(|(sid, symbol_id)| {
                let symbol_id = symbol_id.try_into()?;
                Ok((sid, symbol_id))
            })
            .collect::<InternalCompilerResult<_>>()?;

        self.jir.symbol_strings = self
            .moir
            .symbol_strings
            .into_iter()
            .map(|(symbol_id, sid)| {
                let symbol_id = symbol_id.try_into()?;
                Ok((symbol_id, sid))
            })
            .collect::<InternalCompilerResult<_>>()?;

        let jumplist_cache = JumplistBuilder::new(
            &self.moir.symbol_tables.tables,
            &self.moir.symbol_tables.fields,
            &self.moir.symbol_tables.tokens,
            self.moir.context,
            &self.moir.symbol_tables.varnodes,
            &self.moir.symbol_tables.constructors,
        )
        .transform(self.moir.root_table_id)?;

        self.jir.jumplist_cache = jumplist_cache;
        // TODO: Make alignment configurable such that it can be adjusted depending on the cache line size.
        let (varnodes, group_maps) = transform_varnodes(self.moir.symbol_tables.varnodes, 64)?;
        self.jir.varnode_groups = group_maps.varnode_groups;
        self.jir.varnode_to_group = group_maps.varnode_to_group;
        self.jir.jir_groups = group_maps.jir_groups;

        self.jir.symbol_tables = JIRSymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            constructors: self.moir.symbol_tables.constructors,
            varnodes,
            custom_ops: transform_custom_ops(self.moir.interner, self.moir.symbol_tables.custom_ops)?,
            fields: self.moir.symbol_tables.fields,
        };

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
                    .string(name)
                    .expect("The string has to exist since we saved an id to it.");
                // TODO: Emit a compiler warning instead of a log output
                warn!("Custom op '{name}' is declared but not defined. Ignoring.");
                (id, None)
            }
            MOIRCustomOp::Defined { name, .. } => (
                id,
                Some(JIRCustomOp {
                    name: interner
                        .string(name)
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

/// Constructs varnode groups from a collection of varnodes and creates mapping structures.
///
/// This function takes a collection of varnodes and organizes them into groups of overlapping
/// varnodes. It first creates unsorted groups using `create_unsorted_groups`, then sorts them
/// by offset, and finally converts them into mapping structures.
///
/// The grouping of varnodes is important when transforming or shifting varnodes in an address
/// space, as overlapping varnodes must be shifted together to maintain their relationships.
///
/// # Returns
/// A tuple containing:
/// * A `PrimaryMap` mapping `VarnodeGroupId` to `VarnodeGroup` containing the grouped varnodes
/// * A `SecondaryMap` mapping each `VarNodeId` to its corresponding `VarnodeGroupId`
///
/// # Notes
/// The groups are sorted by their offset values.

fn build_varnode_groups(varnodes: &PrimaryMap<VarNodeId, MOIRVarNode>) -> GroupMaps {
    let mut groups = create_unsorted_groups(varnodes);

    // We want to sort the groups by offset
    groups.sort_by(|a, b| {
        a.as_ref()
            .map(|g| g.offset)
            .cmp(&b.as_ref().map(|g| g.offset))
    });

    build_maps(groups)
}

struct GroupMaps {
    varnode_groups: PrimaryMap<VarnodeGroupId, VarnodeGroup>,
    varnode_to_group: SecondaryMap<VarNodeId, Option<GroupId>>,
    jir_groups: PrimaryMap<GroupId, JIRGroup>,
}

fn build_maps(groups: Vec<Option<VarnodeGroup>>) -> GroupMaps {
    let mut varnode_groups = PrimaryMap::<VarnodeGroupId, VarnodeGroup>::default();
    let mut varnode_to_group = SecondaryMap::<VarNodeId, Option<GroupId>>::default();
    let mut jir_groups = PrimaryMap::<GroupId, JIRGroup>::default();

    for varnode_group in groups.into_iter().flatten() {
        let varnode_group_id = varnode_groups.insert(varnode_group);
        let group_id = jir_groups.insert(JIRGroup::VarnodeGroup(varnode_group_id));

        for varnode_id in &varnode_groups[varnode_group_id].varnodes {
            varnode_to_group[*varnode_id] = Some(group_id);
        }
    }

    GroupMaps {
        varnode_groups,
        varnode_to_group,
        jir_groups,
    }
}

/// Groups overlapping varnodes from a collection into sets (called VarnodeGroups).
///
/// This function analyzes a collection of varnodes and creates groups where each
/// group contains varnodes that overlap with at least one other varnode in that group.
/// When shifting around varnodes in the address space, this is only possible when shifting
/// the entire group a varnode is contained in.
/// # Returns
/// A vector of `Option<VarnodeGroup>` where each entry that is `Some` represents a
/// distinct group of overlapping varnodes. Entries that are `None` represent groups
/// that were merged into other groups.
fn create_unsorted_groups(varnodes: &PrimaryMap<VarNodeId, MOIRVarNode>) -> Vec<Option<VarnodeGroup>> {
    let mut groups = Vec::<Option<VarnodeGroup>>::new();

    for (varnode_id, varnode) in varnodes.items() {
        // check if varnode intersects with any existing intersection group and if so,
        // insert into that group and update group offset and size

        // Get the first group that overlaps if there is one.
        let existing_group = groups.iter().enumerate().find_map(|(id, g)| {
            if let Some(g) = g {
                if overlaps_generic(g, varnode) { Some(id) } else { None }
            } else {
                None
            }
        });

        let Some(existing_group) = existing_group else {
            // No overlapping group found. Create a new one.
            let new_group = VarnodeGroup::new(vec![varnode_id], varnode.offset, varnode.size.try_into().unwrap());
            groups.push(Some(new_group));
            continue;
        };

        // We can now insert the varnode into the existing group.
        if let Some(group) = &mut groups[existing_group] {
            group.insert(varnode_id, varnode);
        }

        // Now, let's check for any additional groups that might overlap and
        // merge them into the first group.
        merge_remaining_overlaps(&mut groups, varnode, existing_group);
    }
    groups
}

/// Merges overlapping varnode groups starting from the specified group ID.
///
/// This function identifies and merges any varnode groups that overlap with the provided
/// varnode, starting from `group_id + 1` onwards. When an overlap is found, the
/// overlapping group is merged into the current group at `group_id`, and its entry
/// in the `groups` vector is set to `None`.
fn merge_remaining_overlaps(
    groups: &mut Vec<Option<VarnodeGroup>>,
    varnode: &MOIRVarNode,
    existing_group_id: usize,
) {
    for group_id in existing_group_id + 1..groups.len() {
        if groups[group_id].is_none() {
            // This group was merged into another. Skip it.
            continue;
        }
        if overlaps_generic(
            groups[group_id]
                .as_ref()
                .expect("The group has to be valid at this point"),
            varnode,
        ) {
            // The group overlaps with the varnode as well.
            // We now take the group out of the collection and merge it with the first group.
            let other_group = groups[group_id].take();
            let existing_group: &mut Option<VarnodeGroup> = &mut groups[existing_group_id];
            if let Some(existing_group) = existing_group
                && let Some(other_group) = other_group
            {
                existing_group.merge(other_group);
            } else {
                unreachable!("The groups must exist at this point.");
            };
        }
    }
}

fn transform_varnodes(
    mut varnodes: PrimaryMap<VarNodeId, MOIRVarNode>,
    cache_line_size: MOIRSize,
) -> InternalCompilerResult<(PrimaryMap<VarNodeId, JIRVarNode>, GroupMaps)> {
    assert_ne!(cache_line_size, 0);
    let cache_alignment = u64::from(cache_line_size);

    let prev_end = varnodes.values().map(MOIRVarNode::end).max().unwrap_or(0);
    debug!("Compacting varnodes. Cache line size is {}. Varnodes occupy {} bytes.", cache_line_size, prev_end);

    let mut group_maps = build_varnode_groups(&varnodes);

    compact_varnode_groups(&mut varnodes, cache_alignment, &mut group_maps.varnode_groups);

    Ok((
        varnodes
            .into_iter()
            .map(|(ident, varnode)| (ident, varnode.into()))
            .collect(),
        group_maps,
    ))
}

fn compact_varnode_groups(
    varnodes: &mut PrimaryMap<VarNodeId, MOIRVarNode>,
    cache_alignment: u64,
    groups: &mut PrimaryMap<VarnodeGroupId, VarnodeGroup>,
) {
    let mut current_offset = 0u64;
    for group in groups.values_mut() {
        let current_cache_line = current_offset.align_down(cache_alignment);
        let end_cache_line = (current_offset + u64::from(group.size))
            .saturating_sub(1)
            .align_down(cache_alignment);
        // If the group crosses a cache line boundary, move it to the next cache line.
        if current_cache_line != end_cache_line {
            current_offset = current_cache_line + cache_alignment;
        }

        // modify group and varnode offsets
        let offset_delta = current_offset as i64 - group.offset as i64;
        if offset_delta != 0 {
            trace_build!("Moving group {:#?} by {:#?}", group, offset_delta);
            group.offset = current_offset;

            // Move all varnodes in this group.
            for varnode_id in &group.varnodes {
                let varnode = varnodes
                    .get_mut(*varnode_id)
                    .expect("This value must exist");
                varnode.offset = (varnode.offset as i64 + offset_delta) as u64;
            }
        } else {
            trace_build!("Group {:#?} is already placed correctly", group);
        }

        current_offset += u64::from(group.size);
    }
    debug!("Varnodes could be compacted compacting yielded size of {:#?}", current_offset);
}

impl From<MOIRVarNode> for JIRVarNode {
    fn from(varnode: MOIRVarNode) -> Self {
        Self {
            space_id: varnode.space,
            offset: varnode.offset,
            size: varnode
                .size
                .try_into()
                .expect("The size must fit into JIRSize"),
        }
    }
}
