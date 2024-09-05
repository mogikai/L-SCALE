use crate::spec::builder::StringId;
use std::fmt::{
    Display,
    Formatter,
};

pub type SASTSize = u8;
pub type SASTLiteral = u128;

#[derive(Debug, Copy, Clone)]
pub struct SASTIdent(pub StringId);

impl Display for SASTIdent {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Ident{}", self.0)
    }
}

#[derive(Debug)]
pub struct SASTString(pub StringId);

impl From<SASTIdent> for StringId {
    fn from(value: SASTIdent) -> Self {
        value.0
    }
}
