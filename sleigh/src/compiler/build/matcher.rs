use chumsky::primitive::todo;
use slotmap::new_key_type;

use crate::spec::moir::field::{
    MOIRFieldId,
};
use crate::spec::moir::table::MOIRTableId;
use crate::spec::moir::token::MOIRTokenId;
use crate::spec::moir::varnode::MOIRVarNodeId;
use crate::spec::MOIRSymbol;

// TODO: for now we assume each token to be of a maximum of 8 bytes, but this doesn't have to be
// the case probably

#[derive(Debug)]
pub enum FieldMatcherConstraint {
    BitPattern(u64),    // r1=0xff00
    Field(MOIRFieldId), // r1=r2
}

/// The simplest kind of matcher. It matches a single field against a value or another field.
#[derive(Debug)]
pub struct FieldMatcher {
    field: MOIRFieldId,
    constraint: FieldMatcherConstraint,
}

impl FieldMatcher {
    pub fn new(
        field: MOIRFieldId,
        constraint: FieldMatcherConstraint,
    ) -> Self {
        Self { field, constraint }
    }

    pub fn r#match(
        &self,
        token_value: u64,
    ) -> Option<Matching> {
        todo!("Pass shift, mask, value and match")
            // match self.constraint {
            //     FieldMatcherConstraint::BitPattern { shift, mask, value } => {
            //         if ((token_value >> shift) & mask) == value {
            //             return Some(self.field);
            //         }

            //         None
            //     }
            //     FieldMatcherConstraint::Field(other) => {
            //         if self.field == other {
            //             return Some(self.field);
            //         }

            //         None
            //     }
            // }
    }
}

// the left-hand side of a matcher expression can be a reference to either a field or a subtable
// TODO do we miss symbols?
#[derive(Debug)]
pub enum MatcherReference {
    Subtable(MOIRTableId),
    Varnode(MOIRVarNodeId),
}

#[derive(Debug)]
pub enum MatcherExpression {
    Field(FieldMatcher), // field=value or field=field
    Linkage(MatcherReference),  // links a local symbol to a global symbol
    Or(Box<MatcherExpression>, Box<MatcherExpression>),  // lhs | rhs
    And(Box<MatcherExpression>, Box<MatcherExpression>), // lhs & rhs
    Concat {
        // need to store tokens for each Concat since they are different (no need
        // to store for Or and And since they always have to use the same)
        lhs: Box<MatcherExpression>,
        lhs_tokens: Vec<(MOIRTokenId, u16)>,
        rhs: Box<MatcherExpression>,
        rhs_tokens: Vec<(MOIRTokenId, u16)>,
    },
}

impl MatcherExpression {
    pub fn r#match(
        &self,
        bytes: &[u8],
    ) -> Option<Vec<Matching>> {
        // bytes are exactly the ones needed for the tokens

        match self {
            MatcherExpression::Field(field_matcher) => {
                let token_value = u64::from_be_bytes(bytes.try_into().expect("Token byte slice has incorrect length"));
                field_matcher.r#match(token_value).map(|m| vec![m])
            },
            MatcherExpression::Linkage(reference) => todo!("Get subtable matcher and return its result"),
            MatcherExpression::Or(lhs, rhs) => {
                lhs.r#match(bytes).or_else(|| rhs.r#match(bytes))
            },
            MatcherExpression::And(lhs, rhs) => {
                if let Some(mut m_lhs) = lhs.r#match(bytes) {
                    if let Some(mut m_rhs) = rhs.r#match(bytes) {
                        m_lhs.append(&mut m_rhs);
                        return Some(m_lhs);
                    }
                }

                None
            },
            MatcherExpression::Concat {
                lhs,
                lhs_tokens,
                rhs,
                rhs_tokens,
            } => {
                todo!()
            },
        }
    }
}

/// Match a token against an expression using field matchers.
#[derive(Debug)]
pub struct Matcher {
    pub(super) expression: Box<MatcherExpression>,

    // this is known before the matcher is run on anything and should only
    // contain fields as well as matcherreference types (i.e. subtables and varnodes)
    pub(super) symbols_used: Vec<MOIRSymbol>,
}

/// A list of all symbols that matched are implicitly given through the matcher
#[derive(Debug)]
pub enum Matching {
    FieldConstant(MOIRFieldId, u64),
    SubtableConstant,
    SubtableVarNode,
}

impl Matcher {
    pub fn new(
        expression: Box<MatcherExpression>,
        symbols_used: Vec<MOIRSymbol>,
    ) -> Self {
        Self {
            expression,
            symbols_used,
        }
    }

    pub fn r#match(
        &self,
        bytes: &[u8],
    ) -> Option<Vec<Matching>> {
        todo!()
    }
}

// #[derive(Error)]
// pub enum MatcherError {
//     #[error("The number of bytes does not fit the matcher.")]
//     SizeError,
// }

// pub type MatcherResult<T = ()> = Result<T, MatcherError>;

new_key_type! { pub struct MatcherId; }
