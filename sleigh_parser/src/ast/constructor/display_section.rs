use crate::{
    Spanned,
    StringId,
};

#[derive(Debug)]
pub struct SASTDisplaySection {
    pub mnemonic: Spanned<String>,
    pub identifiers: Vec<Spanned<StringId>>,
    pub display_str: Spanned<String>,
}
