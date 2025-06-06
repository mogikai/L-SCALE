use std::path::Path;

use mogi_statelearner::graph::{StateLearner, WordId};
use slotmap::SlotMap;

fn main() {
    let mut words = SlotMap::<WordId, String>::default();
    let epsilon = words.insert(String::from(""));
    let word_ids = ["A", "B"]
        .into_iter()
        .map(|word| words.insert(String::from(word)))
        .collect::<Vec<_>>();
    let a = word_ids[0];
    let b = word_ids[1];
    let mut learner = StateLearner::<String>::new(epsilon);

    let initial_row = learner.initial_row();

    learner.add_suffixes([[a], [b]]);
    learner.add_prefix([a, a]);
    let unobserved = learner.unobserved_suffixes(initial_row).collect::<Vec<_>>();

    let row_4 = learner.get_candidate_row_id_from_prefix([b]).unwrap();
    let row_7 = learner.get_candidate_row_id_from_prefix([a, b]).unwrap();

    learner
        .add_observation(row_4, unobserved[1], "test0".to_string())
        .unwrap();
    learner
        .add_observation(row_7, unobserved[1], "test1".to_string())
        .unwrap();

    println!("{}", learner);

    learner.make_consistent();

    println!("{}", learner);

    learner.export(Path::new("test.dot"));
}
