use chumsky::container::Container;
use itertools::Itertools;

use crate::compiler::ast::Spanned;
use crate::compiler::build::constructor::matcher::{
    FieldMatcher,
    Matcher,
    MatcherAtom,
    MatcherId,
};
use crate::compiler::build::trace_build;
use crate::compiler::error::SleighCompilerError;
use crate::compiler::SleighCompilerResult;
use crate::spec::builder::{
    MOIRSymbolId,
    MOIRSymbolTables,
    StringId,
};
use crate::spec::jir::space::{
    JIRSpace,
    JIRSpaceType,
};
use crate::spec::jir::table::{
    JIRTable,
    Jumplist,
    JumplistContainer,
    JumplistEntry,
    JumplistId,
    NextJumplistEntry,
};
use crate::spec::jir::values::JIRVarNode;
use crate::spec::moir::address_space::{
    MOIRSpace,
    MOIRSpaceType,
    SpaceId,
};
use crate::spec::moir::common::MOIRSize;
use crate::spec::moir::constructor::{
    ConstructorId,
    MOIRConstructor,
};
use crate::spec::moir::context::{
    MOIRContext,
    MOIRContextId,
};
use crate::spec::moir::field::{
    FieldId,
    MOIRFieldParent,
};
use crate::spec::moir::table::{
    MOIRTable,
    TableId,
};
use crate::spec::moir::token::{
    MOIRToken,
    TokenId,
};
use crate::spec::moir::varnode::{
    MOIRVarNode,
    VarNodeId,
};
use crate::spec::typed_index_table::TypedIndexTable;
use crate::spec::{
    Ident,
    JIRSymbolId,
    JIRSymbolTables,
};
use std::collections::{
    HashMap,
    HashSet,
};

use self::jumplist::TableTransformer;

#[derive(Debug)]
pub(super) struct MOIRToJIRTransformer {
    moir: MOIRData,
    jir: JIRData,
}

mod jumplist;

#[derive(Debug)]
pub struct JIRData {
    pub symbol_ids: TypedIndexTable<Ident, JIRSymbolId>,
    pub symbol_strings: HashMap<Ident, StringId>,
    pub symbol_tables: JIRSymbolTables,
    pub string_to_ident_mapping: HashMap<StringId, Ident>,
}

#[derive(Debug)]
struct MOIRData {
    symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
    symbol_strings: HashMap<Ident, Spanned<StringId>>,
    symbol_tables: MOIRSymbolTables,
    string_to_ident_mapping: HashMap<StringId, Ident>,
    context: TypedIndexTable<MOIRContextId, MOIRContext>,
    root_table_id: TableId,
}

// TODO remove/move to somewhere else
#[derive(Debug)]
struct ResolvedMatcher {
    // matching
    token_byte_count: Vec<usize>,
    token_field_matchers: Vec<Vec<FieldMatcher>>,
    context_field_matchers: Vec<Vec<FieldMatcher>>,

    // extraction
    fields: Vec<Vec<FieldId>>,
    constructor_id: ConstructorId,
}

impl MOIRToJIRTransformer {
    pub fn new(
        symbol_ids: TypedIndexTable<Ident, MOIRSymbolId>,
        symbol_strings: HashMap<Ident, Spanned<StringId>>,
        symbol_tables: MOIRSymbolTables,
        string_to_ident_mapping: HashMap<StringId, Ident>,
        context: TypedIndexTable<MOIRContextId, MOIRContext>,
        root_table_id: TableId,
    ) -> Self {
        Self {
            moir: MOIRData {
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
                string_to_ident_mapping: Default::default(),
            },
        }
    }

    pub fn transform(mut self) -> SleighCompilerResult<JIRData> {
        self.jir.symbol_ids = self
            .moir
            .symbol_ids
            .into_iter()
            .map(|(ident, symbol_id)| {
                let symbol_id = match symbol_id {
                    MOIRSymbolId::Undefined => todo!("Generate error for undefined symbol"),
                    MOIRSymbolId::Macro(id) => JIRSymbolId::Macro(id),
                    MOIRSymbolId::Space(id) => JIRSymbolId::Space(id),
                    MOIRSymbolId::Table(id) => JIRSymbolId::Table(id),
                    MOIRSymbolId::Token(id) => JIRSymbolId::Token(id),
                    MOIRSymbolId::Field(id) => JIRSymbolId::Field(id),
                    MOIRSymbolId::VarNode(id) => JIRSymbolId::VarNode(id),
                    MOIRSymbolId::BitrangeRegister(id) => JIRSymbolId::BitrangeRegister(id),
                    MOIRSymbolId::CustomOp(id) => JIRSymbolId::CustomOp(id),
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
        self.jir.symbol_tables = JIRSymbolTables {
            spaces: transform_spaces(self.moir.symbol_tables.spaces)?,
            tables: transform_tables(
                self.moir.symbol_tables.tables,
                self.moir.root_table_id,
                &self.moir.symbol_tables.tokens,
                &self.moir.context,
                &self.moir.symbol_tables.varnodes,
                &self.moir.symbol_tables.constructors,
            )?,
            constructors: self.moir.symbol_tables.constructors,
            varnodes: transform_varnodes(self.moir.symbol_tables.varnodes)?,
        };
        self.jir.string_to_ident_mapping = self.moir.string_to_ident_mapping;
        Ok(self.jir)
    }
}

/// Transform tables by combining every root [Matcher] into multiple jumplists.
fn transform_tables(
    tables: TypedIndexTable<TableId, MOIRTable>,
    root_table_id: TableId,
    tokens: &TypedIndexTable<TokenId, MOIRToken>,
    context: &TypedIndexTable<MOIRContextId, MOIRContext>,
    varnodes: &TypedIndexTable<VarNodeId, MOIRVarNode>,
    constructors: &TypedIndexTable<ConstructorId, MOIRConstructor>,
) -> SleighCompilerResult<TypedIndexTable<TableId, JIRTable>> {
    let mut transformer = TableTransformer::new(&tables, tokens, context, varnodes, constructors);
    transformer.transform(root_table_id)
}

fn transform_spaces(
    spaces: TypedIndexTable<SpaceId, MOIRSpace>
) -> SleighCompilerResult<TypedIndexTable<SpaceId, JIRSpace>> {
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
                address_size: space.address_size,
                wordsize: space.wordsize,
            };
            (ident, space)
        })
        .collect())
}

fn transform_varnodes(
    varnodes: TypedIndexTable<VarNodeId, MOIRVarNode>
) -> SleighCompilerResult<TypedIndexTable<VarNodeId, JIRVarNode>> {
    Ok(varnodes
        .into_iter()
        .map(|(ident, varnode)| {
            let varnode = JIRVarNode {
                space_id: varnode.space,
                offset: varnode.offset,
                size: varnode.size,
            };
            (ident, varnode)
        })
        .collect())
}

// /// Remove all jumplists in `jumplist` from `jumplist_cache` as well as `sublist_cache` and
// /// rewrite data structures to fix dangling references. Deleted entries will be replaced
// /// by `replace_id`.
// fn remove_jumplists(
//     jumplists: &[JumplistId],
//     replace_id: JumplistId,
//     jumplist_cache: &mut TypedIndexTable<JumplistId, Jumplist>,
//     sublist_cache: &mut HashMap<(TableId, MOIRSize), JumplistId>,
// ) {
//     trace_build!("Removing jumplists {:?}", jumplists);
//     let predicate = |jumplist_id: JumplistId, jumplist: &Jumplist| jumplists.contains(&jumplist_id);
//     let mut rewrite_rule = |jumplist: &mut Jumplist, permutation: &Vec<Option<JumplistId>>| {
//         let permuted_replace = permutation[usize::try_from(replace_id).unwrap()].expect("The replacement id must not be removed");
//         eprintln!("permutation = {:?}", permutation);
//         jumplist.for_entries_mut(&mut |entry| {
//             entry.same_token_jumplist.iter_mut().for_each(|id| {
//                 match permutation[usize::try_from(*id).unwrap()] {
//                     Some(new_id) => *id = new_id,
//                     None => *id = permuted_replace,
//                 }
//             });
//             if let NextJumplistEntry::Jumplist(id) = &mut entry.next_entry {
//                 match permutation[usize::try_from(*id).unwrap()] {
//                     Some(new_id) => *id = new_id,
//                     None => *id = permuted_replace,
//                 }
//             }
//         });

//         sublist_cache.values_mut().for_each(|id| *id = permutation[usize::try_from(*id).unwrap()].expect(""));
//     };

//     jumplist_cache.remove_and_rewrite(predicate, &mut rewrite_rule);
// }
