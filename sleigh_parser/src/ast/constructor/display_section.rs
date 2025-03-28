use crate::{
    Spanned,
    StringId,
};

#[derive(Debug, Clone)]
pub enum SASTDisplayFragment {
    Literal(String),
    Identifier(StringId),
    Whitespace,
    Hat,
}

#[derive(Debug)]
pub struct SASTDisplaySection {
    pub display_str: Spanned<String>,

    /// The display section consists of literals and identifiers which need to be treated
    /// separately for constructing the final display string. If the constructor is part of the
    /// root table, the first fragment will be treated as an identifier by the parser which needs
    /// to be fixed later on.
    pub fragments: Vec<Spanned<SASTDisplayFragment>>,
}

impl SASTDisplaySection {
    /// Returns a list of all identifiers in the display string.
    pub fn identifiers(&self) -> Vec<Spanned<StringId>> {
        self.fragments
            .iter()
            .filter_map(|fragment| match fragment.value() {
                SASTDisplayFragment::Literal(_) | SASTDisplayFragment::Whitespace | SASTDisplayFragment::Hat => None,
                SASTDisplayFragment::Identifier(string_id) => Some(Spanned::new(*string_id, fragment.span().clone())),
            })
            .collect::<Vec<_>>()
    }
}
