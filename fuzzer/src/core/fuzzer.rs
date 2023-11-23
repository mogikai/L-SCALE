use std::collections::BTreeMap;

use petgraph::graph::{
    DiGraph,
    NodeIndex,
};

use crate::api::observer::Observer;

struct State {}

pub struct Fuzzer {
    observers: Vec<Box<dyn Observer>>,
    state_graph: DiGraph<(), ()>,
    states: BTreeMap<NodeIndex, State>,
}

impl Fuzzer {
    pub fn run(&mut self) {
        // Do setup stuff
        loop {}
    }
}
