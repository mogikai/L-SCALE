use mogi_statelearner::graph::{
    ObservationValue,
    StateLearner,
    WordId,
};
use petgraph::dot::{
    Config,
    Dot,
};
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
use petgraph::prelude::EdgeRef;
use slotmap::{
    SlotMap,
    new_key_type,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{
    Path,
    PathBuf,
};
use std::thread::sleep;
use std::time::Duration;

new_key_type! { struct StateKey; }

#[derive(Debug)]
struct State {
    name: String,
}

#[derive(Debug)]
struct Transition {
    input: String,
    target: NodeIndex,
    observation: i32,
}

#[derive(Debug)]
struct StateMachine {
    states: DiGraph<State, Transition>,
    initial_state: NodeIndex,
    current_state: NodeIndex,
    defined_inputs: HashMap<NodeIndex, Vec<String>>,
    error_state: NodeIndex,
}

impl StateMachine {
    fn new() -> Self {
        let mut states = DiGraph::new();
        let initial_state = states.add_node(State {
            name: "Initial".to_string(),
        });
        let error_state = states.add_node(State {
            name: "Error".to_string(),
        });

        StateMachine {
            states,
            initial_state,
            current_state: initial_state,
            defined_inputs: HashMap::new(),
            error_state,
        }
    }

    fn add_state(
        &mut self,
        name: &str,
    ) -> NodeIndex {
        self.states.add_node(State { name: name.to_string() })
    }

    fn add_transition(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        input: &str,
        observation: i32,
    ) {
        // Store the input as defined for this state
        self.defined_inputs
            .entry(from)
            .or_insert_with(Vec::new)
            .push(input.to_string());

        self.states.add_edge(
            from,
            to,
            Transition {
                input: input.to_string(),
                target: to,
                observation,
            },
        );
    }

    fn add_error_transitions(&mut self) {
        // Define a special "any_other" input that represents unhandled inputs
        let error_input = "any_other";
        let error_observation = -1;

        // For each state
        let all_nodes: Vec<NodeIndex> = self.states.node_indices().collect();

        for state_idx in all_nodes {
            // Skip adding error transitions from the error state
            if state_idx == self.error_state {
                continue;
            }

            // Add transition to error state for unhandled inputs
            self.states.add_edge(
                state_idx,
                self.error_state,
                Transition {
                    input: error_input.to_string(),
                    target: self.error_state,
                    observation: error_observation,
                },
            );
        }

        // Add self-loop on error state
        self.states.add_edge(
            self.error_state,
            self.error_state,
            Transition {
                input: error_input.to_string(),
                target: self.error_state,
                observation: error_observation,
            },
        );
    }

    fn process_input(
        &mut self,
        input: &str,
    ) -> Option<i32> {
        let current = self.current_state;

        // First try to find a specific transition for this input
        for edge in self.states.edges(current) {
            if edge.weight().input == input {
                self.current_state = edge.target();
                return Some(edge.weight().observation);
            }
        }

        // If no specific transition is found, use the error transition
        for edge in self.states.edges(current) {
            if edge.weight().input == "any_other" {
                self.current_state = edge.target();
                return Some(edge.weight().observation);
            }
        }

        // This should never happen if error transitions are properly set up
        None
    }

    fn reset(&mut self) {
        self.current_state = self.initial_state;
    }

    fn is_in_error_state(&self) -> bool {
        self.current_state == self.error_state
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct MyI32(i32);

impl ObservationValue for MyI32 {}

fn main() {
    let mut machine = StateMachine::new();
    let error_state = machine.error_state;

    // Create 10 states
    let s0 = machine.current_state; // Initial state
    let s1 = machine.add_state("State1");
    let s2 = machine.add_state("State2");
    let s3 = machine.add_state("State3");
    let s4 = machine.add_state("State4");
    let s5 = machine.add_state("State5");
    let s6 = machine.add_state("State6");
    let s7 = machine.add_state("State7");
    let s8 = machine.add_state("State8");
    let s9 = machine.add_state("State9");

    // Add transitions with observations
    machine.add_transition(s0, s1, "start", 10);
    machine.add_transition(s1, s2, "next", 20);
    machine.add_transition(s2, s3, "continue", 30);
    machine.add_transition(s3, s4, "proceed", 40);
    machine.add_transition(s4, s5, "advance", 50);
    machine.add_transition(s5, s6, "forward", 60);
    machine.add_transition(s6, s7, "progress", 70);
    machine.add_transition(s7, s8, "move", 80);
    machine.add_transition(s8, s9, "final", 90);
    machine.add_transition(s9, s0, "reset", 0);

    // Add some cyclic transitions
    machine.add_transition(s5, s3, "back", 35);
    machine.add_transition(s7, s4, "return", 45);
    machine.add_transition(s9, s6, "jump", 65);

    // Add error transitions for all states
    machine.add_error_transitions();

    let words = [
        "start", "next", "continue", "proceed", "advance", "forward", "progress", "move", "final", "reset", "back",
        "return", "jump",
    ];

    let mut word_map = SlotMap::<WordId, String>::default();
    for word in words.iter() {
        word_map.insert(word.to_string());
    }

    let word_ids = word_map.keys().collect::<Vec<_>>();

    let mut learner = StateLearner::<MyI32>::new();
    for word in word_ids.iter().copied() {
        learner.add_prefix([word]);
        learner.add_suffix([word]);
    }

    println!("{}", learner);

    while let Some(row_id) = learner.next_row_with_missing_observations() {
        let prefix = learner.get_row_prefix(row_id).collect::<Vec<_>>();
        let unobserved_suffixes = learner.unobserved_suffixes(row_id).collect::<Vec<_>>();

        for suffix_id in unobserved_suffixes.iter().copied() {
            machine.reset();
            for prefix_word_id in prefix.iter().copied() {
                let prefix_word = &word_map[prefix_word_id];
                machine.process_input(prefix_word);
            }

            let mut observation = 1;
            for suffix_word_id in learner.get_suffix_words(suffix_id) {
                let suffix_word = &word_map[suffix_word_id];
                observation *= machine.process_input(suffix_word).unwrap();
            }

            learner
                .add_observation(row_id, suffix_id, MyI32(observation))
                .unwrap();
            learner.make_closed();
            learner.make_consistent();
        }

        println!("{}", learner);
        learner.export(&PathBuf::from("statelearner/state_test.dot"), &word_map);
    }

    let dot = Dot::with_config(&machine.states, &[]);
    let mut file = File::options()
        .truncate(true)
        .write(true)
        .create(true)
        .open("statelearner/state_machine.dot")
        .unwrap();
    file.write_fmt(format_args!("{:?}", dot)).unwrap();
    learner.export(&PathBuf::from("statelearner/state_test.dot"), &word_map);
}
