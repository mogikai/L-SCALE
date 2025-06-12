use crate::row::ObservationRowId;
use crate::{
    StateLearner,
    SuffixId,
    SymbolId,
};
use itertools::Itertools;
use slotmap::Key;
use std::fmt::{
    Debug,
    Display,
    Write,
};

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

impl Display for SymbolId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Sym({:?})", self.data())
    }
}

impl<T> Display for StateLearner<T>
where
    T: Debug,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        writeln!(f, "StateLearner:")?;
        write!(f, "Suffixes:")?;
        for suffix in self.suffixes.values() {
            write!(f, " {:?}, ", suffix)?;
        }
        write!(f, "\nRows:\n")?;

        let mut header_rows = Vec::new();

        for symbol_id in 0..self.suffixes.values().map(Vec::len).max().unwrap_or(0) {
            let mut row_string = String::new();

            for suffix_symbols in self.suffixes.values() {
                let suffix_symbol = match suffix_symbols.get(symbol_id) {
                    None => "".into(),
                    Some(suffix_symbol) => format!("{}", suffix_symbol),
                };

                write!(row_string, "{:^21}", suffix_symbol)?;
            }

            header_rows.push(format!("{:<60}{}", "", row_string));
        }

        let header_width = header_rows.last().map(|row| row.len()).unwrap_or(0);
        let header_str = header_rows.into_iter().rev().join("\n");
        writeln!(f, "{}", header_str)?;
        writeln!(f, "{}", std::iter::repeat("-").take(header_width).join(""))?;

        for (row_id, row) in self.observation_rows.iter() {
            let mut row_string = String::new();
            if row.is_state() {
                if let Some(states) = self.row_to_state.get(&row_id)
                    && let Some(first_state) = states.iter().next()
                {
                    write!(row_string, "{:<16}", format!("<{first_state:?}>"))?;
                } else {
                    write!(row_string, "{:<16}", "<state>")?;
                }
            } else {
                write!(row_string, "{:<16}", "<candidate>")?;
            }
            row.fmt_debug_row(&format!("{}", row_id), &mut row_string, &self.suffixes)?;
            if row.is_state() {
                writeln!(f, "{}", ansi_term::Colour::Green.paint(row_string))?;
                if let Some(states) = self.row_to_state.get(&row_id) {
                    for state in states.iter().skip(1) {
                        writeln!(f, "{:<16}", ansi_term::Colour::Green.paint(format!("<{state:?}>")))?;
                    }
                }
            } else {
                writeln!(f, "{}", row_string)?;
            }
        }

        Ok(())
    }
}
