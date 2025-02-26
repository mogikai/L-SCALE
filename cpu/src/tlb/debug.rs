use crate::tlb::entry::TLBEntry;
use crate::tlb::{
    PAGE_OFFSET_BITS,
    TLB,
    TLB_INDEX_BITS,
};
use mogi_utilities::addresses::EVAddr;
use std::fmt::{
    Debug,
    Formatter,
};

impl<PageDataT> Debug for TLB<PageDataT> {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "TLB: {{")?;
        fn write_color<T>(
            f: &mut Formatter<'_>,
            i: usize,
            entry: &TLBEntry<T>,
            color: ansi_term::Colour,
        ) -> std::fmt::Result {
            if i % 10 == 0 {
                writeln!(f, "")?;
            }
            let str = if entry.target.is_some() {
                format!(
                    "{:?}->[{:?}], ",
                    EVAddr::<u8>::from(i as u64 | entry.tag << (PAGE_OFFSET_BITS + TLB_INDEX_BITS)),
                    entry
                )
            } else {
                String::from("E, ")
            };
            write!(f, "{}", color.paint(str))
        }
        for (i, entry) in self.read_entries.iter().enumerate() {
            write_color(f, i, entry, ansi_term::Colour::Green)?;
        }
        for (i, entry) in self.write_entries.iter().enumerate() {
            write_color(f, i, entry, ansi_term::Colour::Blue)?;
        }
        write!(f, "\n}}")
    }
}
