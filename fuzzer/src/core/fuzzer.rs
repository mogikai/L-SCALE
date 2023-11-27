use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use log::debug;
use petgraph::graph::{
    DiGraph,
    NodeIndex,
};
use serde::{
    Deserialize,
    Serialize,
};
use slotmap::{
    new_key_type,
    SlotMap,
};

use fuzzyemu::emulator::abstractions::system::SystemControl;
use fuzzyemu::emulator::interface::emulator::{
    Emulator,
    ExitStatus,
};

use crate::api::observer::Observer;
use crate::observers::emulator::edge::EdgeObserver;

/// Testcases have an associated input and some metadata. Each testcase belongs to one state.
/// The input data may be shared across testcases.
#[derive(Debug, Serialize, Deserialize)]
struct TestCase {
    /// How often the testcase was used as basis for an execution.
    executions: usize,
    input: Input,
}

impl TestCase {
    pub fn new(input: Input) -> Self {
        Self { executions: 0, input }
    }
}

/// An emulator input. The input may have multiple streams for each input component of the emulator.
/// The fuzzer will prepare the inputs such that the number of streams matches the amount
/// of streams that are currently awaiting input in the emulator.
/// The mutators can then decide whether or not to fulfil these demands.
/// At least one stream has to contain input, otherwise the emulation will be a no-op.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Input {
    /// There may be multiple tcp streams. Each stream is a vector of input data ids that point to an input blob.
    tcp_streams: Vec<InputDataId>,
}

impl Input {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    // TODO: Provide a utility to build initial inputs for fuzzing from raw streams.
    //       A GUI might be helpful for this. (PSE project?)
}

/// An object that contains arbitrary bytes that can be used in any kind of input component.
#[derive(Debug, Default)]
struct InputData {
    data: Vec<u8>,
}

impl From<Vec<u8>> for InputData {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

new_key_type! { struct InputDataId; }

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    label: Option<String>,
    /// How often the target was executed in this state.
    executions: usize,
    /// The testcases associated with this state.
    corpus: Vec<TestCase>,
}

impl State {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn new_with_label(label: String) -> Self {
        Self {
            label: Some(label),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default)]
pub struct FuzzerState {
    state_graph: DiGraph<(), ()>,
    states: BTreeMap<NodeIndex, State>,
    /// A map of various input data blobs that are used by inputs. Multiple inputs may
    /// refer to the same input data blob.
    input_data: SlotMap<InputDataId, InputData>,
    /// The path through the state graph that the fuzzer has currently taken.
    /// The fuzzer can backtrack along this path.
    state_path: Vec<NodeIndex>,
}

trait DebuggableObserver: Observer + Debug {}

impl<T> DebuggableObserver for T where T: Observer + Debug {}

#[derive(Debug, Default)]
pub struct Fuzzer<EmulatorT> {
    state: FuzzerState,
    observers: Vec<Rc<RefCell<dyn DebuggableObserver>>>,
    emulator: EmulatorT,
    context_path: PathBuf,
}

impl<EmulatorT> Fuzzer<EmulatorT>
where
    EmulatorT: Emulator,
{
    pub fn new(
        emulator: EmulatorT,
        context_path: PathBuf,
    ) -> Self {
        Self {
            state: Default::default(),
            observers: Default::default(),
            emulator,
            context_path,
        }
    }

    pub fn run(&mut self) {
        let do_fuzzing = Arc::new(AtomicBool::new(true));
        {
            let do_fuzzing = do_fuzzing.clone();
            ctrlc::set_handler(move || {
                do_fuzzing.store(false, Ordering::SeqCst);
            })
            .expect("Error setting handler");
        }
        self.setup();
        while do_fuzzing.load(Ordering::SeqCst) {
            self.fuzz();
        }
    }

    fn setup(&mut self) {
        self.load_initial_state();
        let edge_observer = Rc::new(RefCell::new(EdgeObserver::new(self.emulator.system().cpu())));
        self.observers.push(edge_observer.clone());
        // Do setup stuff
        // Run emulator without inputs until an input is requested.
        let res = self.emulator.run(None, None).unwrap();
        assert!(matches!(res, ExitStatus::Yielded));
    }

    fn load_initial_state(&mut self) {
        let node_id = self.state.state_graph.add_node(());
        let mut state = State::new_with_label("Initial".to_string());
        let mut corpus_dir = self.context_path.clone();
        corpus_dir.push("corpus");
        for file in corpus_dir.read_dir().unwrap() {
            if let Ok(file) = file {
                if file.file_type().unwrap().is_file() {
                    let data = fs::read(file.path()).unwrap();
                    let input_data_id = self.state.input_data.insert(InputData::from(data));
                    // TODO: Refactor this into more generic code and don't access private fields.
                    let mut input = Input::new();
                    input.tcp_streams.push(input_data_id);
                    let testcase = TestCase::new(input);
                    state.corpus.push(testcase);
                }
            }
        }
        self.state.states.insert(node_id, state);
        debug!("{:?}", self.state);
        debug!("ASDF");
    }

    fn fuzz(&mut self) {
        for observer in self.observers.iter_mut() {
            observer.borrow_mut().clear();
        }
        // Select state
        // Select input from state corpus
        // Do mutations
        // Execute the target with the input(s)
        //

        for observer in self.observers.iter_mut() {
            observer.borrow_mut().gather();
        }
        sleep(Duration::from_millis(100));
        debug!("Current fuzzer state: {:?}", self.state);
    }
}
