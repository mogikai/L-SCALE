use itertools::Itertools;
use mogi_statelearner::graph::{
    ObservationValue,
    WordId,
};
use mogi_statelearner::StateLearner;
use petgraph::dot::{
    Config,
    Dot,
};
use petgraph::graph::DiGraph;
use petgraph::visit::NodeRef;
use rand::prelude::*;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Start,
    Login,
    Authenticated,
    Home,
    ViewProfile,
    EditProfile,
    Search,
    Settings,
    Notification,
    Logout,
}

impl std::fmt::Display for State {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Behavior {
    Welcome,
    AskCredentials,
    DenyAccess,
    ShowHome,
    ShowProfile,
    EditProfile,
    SearchResult,
    ShowSettings,
    ShowNotifications,
    Goodbye,
    Invalid,
}

impl ObservationValue for Behavior {}

#[derive(Clone)]
pub struct Transition {
    pub from: State,
    pub input: &'static str,
    pub to: State,
    pub behavior: Behavior,
}

pub struct StateMachine {
    current_state: State,
    transitions: HashMap<(State, &'static str), (State, Behavior)>,
    transition_table: Vec<Transition>,
}

impl StateMachine {
    pub fn new() -> Self {
        let transitions = vec![
            Transition {
                from: State::Start,
                input: "login",
                to: State::Login,
                behavior: Behavior::AskCredentials,
            },
            Transition {
                from: State::Login,
                input: "success",
                to: State::Authenticated,
                behavior: Behavior::Welcome,
            },
            Transition {
                from: State::Login,
                input: "fail",
                to: State::Start,
                behavior: Behavior::DenyAccess,
            },
            Transition {
                from: State::Authenticated,
                input: "home",
                to: State::Home,
                behavior: Behavior::ShowHome,
            },
            Transition {
                from: State::Authenticated,
                input: "profile",
                to: State::ViewProfile,
                behavior: Behavior::ShowProfile,
            },
            Transition {
                from: State::ViewProfile,
                input: "edit",
                to: State::EditProfile,
                behavior: Behavior::EditProfile,
            },
            Transition {
                from: State::EditProfile,
                input: "save",
                to: State::ViewProfile,
                behavior: Behavior::ShowProfile,
            },
            Transition {
                from: State::Home,
                input: "search",
                to: State::Search,
                behavior: Behavior::SearchResult,
            },
            Transition {
                from: State::Authenticated,
                input: "settings",
                to: State::Settings,
                behavior: Behavior::ShowSettings,
            },
            Transition {
                from: State::Authenticated,
                input: "notify",
                to: State::Notification,
                behavior: Behavior::ShowNotifications,
            },
            Transition {
                from: State::Authenticated,
                input: "logout",
                to: State::Logout,
                behavior: Behavior::Goodbye,
            },
            Transition {
                from: State::Logout,
                input: "login",
                to: State::Login,
                behavior: Behavior::AskCredentials,
            },
        ];

        let mut map = HashMap::new();
        for t in &transitions {
            map.insert((t.from, t.input), (t.to, t.behavior.clone()));
        }

        Self {
            current_state: State::Start,
            transitions: map,
            transition_table: transitions,
        }
    }

    pub fn reset(&mut self) {
        self.current_state = State::Start;
    }

    /// Advances the state machine on the given input, returning the triggered behavior.
    pub fn process(
        &mut self,
        input: &str,
    ) -> Behavior {
        if let Some(&(next_state, ref behavior)) = self.transitions.get(&(self.current_state, input)) {
            self.current_state = next_state;
            behavior.clone()
        } else {
            Behavior::Invalid
        }
    }

    pub fn current_state(&self) -> State {
        self.current_state
    }

    /// Exports the state graph as a DOT file using petgraph.
    pub fn export_dot(
        &self,
        filename: &str,
    ) {
        let mut graph = DiGraph::<State, String>::new();
        let mut nodes = HashMap::new();

        // Add all states as nodes
        for state in [
            State::Start,
            State::Login,
            State::Authenticated,
            State::Home,
            State::ViewProfile,
            State::EditProfile,
            State::Search,
            State::Settings,
            State::Notification,
            State::Logout,
        ] {
            let node = graph.add_node(state);
            nodes.insert(state, node);
        }

        // Add edges for transitions
        for t in &self.transition_table {
            let from = nodes[&t.from];
            let to = nodes[&t.to];
            let label = format!("{} / {:?}", t.input, t.behavior);
            graph.add_edge(from, to, label);
        }

        let dot = Dot::with_attr_getters(&graph, &[Config::EdgeNoLabel], &|_, _| String::new(), &|_, edge_ref| {
            format!("label = \"{}\"", edge_ref.weight())
        });

        std::fs::write(filename, format!("{}", dot)).expect("Failed to write dot file");
    }
}

// Example usage:
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sm = StateMachine::new();

    let mut words = SlotMap::<WordId, String>::default();
    let epsilon = words.insert("".to_string());

    for transition in sm.transition_table.iter() {
        words.insert(transition.input.to_string());
    }

    let word_ids = words.keys().collect::<Vec<_>>();

    let mut learner = StateLearner::<Behavior>::new(epsilon);

    for word in words.keys() {
        learner.add_prefix([word]);
        learner.add_suffix([word]);
    }
    println!("{}", learner);

    // Generate and process random prefixes
    let mut rng = rand::thread_rng();
    for _ in 0..5 {
        let prefix_len = rng.random_range(1..=5);
        let prefix_vec: Vec<WordId> = (1..prefix_len)
            .map(|_| word_ids[rng.random_range(1..word_ids.len())])
            .collect();

        // Process prefix in state machine
        sm.reset();
        learner.add_prefix(prefix_vec.iter().copied());
        let mut prefix = prefix_vec.iter().copied().peekable();
        println!("{}", learner);
        while let Some(word_id) = prefix.next() {
            let behavior = sm.process(&words[word_id]);
            if prefix.peek().is_none() {
                let suffix = learner.add_suffix([word_id]);
                let row = learner
                    .get_candidate_row_id_from_prefix(prefix_vec.iter().copied())
                    .unwrap();
                learner.add_observation(row, suffix, behavior).unwrap();
                learner.make_closed();
                learner.make_consistent();
            }
        }
    }

    println!("{}", learner);

    // Export graph
    sm.export_dot("statelearner/state_machine.dot");
    learner.export(Path::new("statelearner/test.dot"));
    Ok(())
}
