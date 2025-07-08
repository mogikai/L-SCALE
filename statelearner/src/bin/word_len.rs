use itertools::Itertools;
use mogi_statelearner::{
    ObservationValue,
    StateLearner,
    SymbolId,
};
use slotmap::SlotMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct MyI32(i32);

impl Display for MyI32 {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ObservationValue for MyI32 {}

fn main() {
    let symbols = ["A", "B"];

    let mut word_map = SlotMap::<SymbolId, String>::default();
    let symbol_ids = symbols
        .iter()
        .map(|symbol| word_map.insert(symbol.to_string()))
        .collect::<Vec<_>>();

    let mut learner = StateLearner::default();
    println!("{}", learner);
    for symbol in symbol_ids.iter().copied() {
        learner.add_prefix([symbol]);
        learner.add_suffix([symbol]);
    }

    for (a, b) in symbol_ids
        .iter()
        .copied()
        .cartesian_product(symbol_ids.iter().copied())
    {
        learner.add_prefix([a, b]);
        learner.add_suffix([a, b]);
    }

    println!("{}", learner);

    while let Some((row_id, prefix)) = learner.next_row_with_missing_observations(None) {
        let unobserved_suffixes = learner.unobserved_suffixes(row_id).collect::<Vec<_>>();

        for suffix_id in unobserved_suffixes.iter().copied() {
            let mut num_as = 0;
            #[allow(unused_variables)]
            let mut num_bs = 0;
            for symbol_id in prefix.clone().chain(learner.get_suffix_symbols(suffix_id)) {
                let symbol = word_map[symbol_id].as_str();
                match symbol {
                    "A" => num_as += 1,
                    "B" => num_bs += 1,
                    _ => {}
                }
            }

            learner
                .add_observation(row_id, suffix_id, MyI32((num_as % 4 == 3) as i32))
                .unwrap();
            learner.make_consistent_and_closed(|| true);
            sleep(Duration::from_millis(10));
        }

        println!("{}", learner);
        learner.export(&PathBuf::from("statelearner/state_test.dot"), &word_map);
    }

    learner.export(&PathBuf::from("statelearner/state_test.dot"), &word_map);
}
