use super::common::SASTIdent;
use super::Spanned;

#[derive(Debug)]
pub struct SASTVariable {
    pub fields: Vec<Spanned<SASTIdent>>,
    pub varnodes: Vec<Spanned<SASTIdent>>,
}

#[derive(Debug)]
pub struct SASTValue {
    pub fields: Vec<Spanned<SASTIdent>>,
    pub values: Vec<Spanned<u64>>,
}

#[derive(Debug)]
pub struct SASTName {
    pub fields: Vec<Spanned<SASTIdent>>,
    pub strings: Vec<Spanned<SASTIdent>>,
}

#[derive(Debug)]
pub enum SASTAttach {
    SASTVariable(Spanned<SASTVariable>),
    SASTValue(Spanned<SASTValue>),
    SASTName(Spanned<SASTName>),
}
