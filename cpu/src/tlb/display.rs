use crate::tlb::AccessType;
use std::fmt::{
    Display,
    Formatter,
};

impl Display for AccessType {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AccessType::Load => "load",
                AccessType::Store => "store",
            }
        )
    }
}
