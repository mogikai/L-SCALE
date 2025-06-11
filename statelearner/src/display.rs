use crate::row::ObservationRowId;
use crate::{
    SuffixId,
    WordId,
};
use slotmap::Key;
use std::fmt::Display;

impl Display for ObservationRowId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Row({:?})", self.data())
    }
}

impl Display for SuffixId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Suffix({:?})", self.data())
    }
}

impl Display for WordId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Word({:?})", self.data())
    }
}
