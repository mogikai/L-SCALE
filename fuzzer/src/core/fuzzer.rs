use std::cell::RefCell;
use std::collections::{
    BTreeMap,
    BTreeSet,
};
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

use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
use fuzzyemu::emulator::abstractions::network::tcp_system::TCPSystem;
use fuzzyemu::emulator::abstractions::network::{
    IPAddress,
    SocketAddress,
    SocketId,
};
use fuzzyemu::emulator::abstractions::system::SystemControl;
use fuzzyemu::emulator::interface::emulator::{
    Emulator,
    ExitStatus,
};
use fuzzyemu::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};

use crate::api::observer::Observer;
use crate::api::reducer::Reducer;
use crate::feedback::edge::InterestingEdgesReducer;
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
    /// There may be multiple tcp streams. Each stream either is empty (None) or points to a data blob via the
    /// InputDataId.
    tcp_streams: Vec<Option<InputDataId>>,
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

trait DebuggableReducer: Reducer + Debug {}

impl<T> DebuggableReducer for T where T: Reducer + Debug {}

#[derive(Debug, Default)]
struct TcpInfo {
    listeners: Vec<SocketId>,
    streams_writable: Vec<SocketId>,
}

/// Contains information about the currently active listeners or streams of the emulator.
#[derive(Debug, Default)]
struct EmulatorInputInfo {
    tcp: TcpInfo,
}

#[derive(Debug, Default)]
pub struct Fuzzer<EmulatorT> {
    state: FuzzerState,
    observers: Vec<Rc<RefCell<dyn DebuggableObserver>>>,
    is_interesting_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    is_crash_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    emulator: EmulatorT,
    context_path: PathBuf,
    input_info: EmulatorInputInfo,
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
            is_interesting_reducers: Default::default(),
            is_crash_reducers: Default::default(),
            emulator,
            context_path,
            input_info: Default::default(),
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
        let edge_observer = Rc::new(RefCell::new(EdgeObserver::new(self.emulator.system().cpu())));
        let edge_reducer = Rc::new(RefCell::new(InterestingEdgesReducer::new(edge_observer.clone())));
        self.observers.push(edge_observer);
        self.is_interesting_reducers.push(edge_reducer);
        // Do setup stuff
        // Run emulator without inputs until an input is requested.
        let res = self.emulator.run(None, None).unwrap();
        // We make a snapshot now, so that we can reset to the first input request.
        self.emulator.snapshot().unwrap();
        // Reduce once, so that any state of the reducers can be filled with the initial observations.
        assert!(matches!(res, ExitStatus::Yielded));
        self.gather_observations();
        self.reduce_is_interesting();
        self.clear_observers();
        self.load_initial_state();
        self.evaluate_initial_state();
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
                    input.tcp_streams.push(Some(input_data_id));
                    let testcase = TestCase::new(input);
                    state.corpus.push(testcase);
                }
            }
        }
        self.state.states.insert(node_id, state);
        self.state.state_path.push(node_id);
    }

    fn update_input_info(&mut self) {
        self.input_info.tcp.listeners.clear();
        self.input_info.tcp.streams_writable.clear();
        for (&sock_id, sock) in self.emulator.system().tcp().borrow().sockets() {
            if sock
                .interest()
                .contains(FileOperationsReady::Read | FileOperationsReady::ReadNormal)
            {
                if sock.is_listener() {
                    self.input_info.tcp.listeners.push(sock_id);
                } else {
                    self.input_info.tcp.streams_writable.push(sock_id);
                }
            }
        }
    }

    fn evaluate_initial_state(&mut self) {
        self.update_input_info();
        let current_state = self.current_state_id();
        for testcase in self
            .state
            .states
            .get_mut(&current_state)
            .unwrap()
            .corpus
            .iter_mut()
        {
            // Check if we need to instruct the emulator to open new streams
            let requested_streams = testcase
                .input
                .tcp_streams
                .len()
                .saturating_sub(self.input_info.tcp.streams_writable.len());
            let mut tcp = self.emulator.system().tcp().borrow_mut();
            for _ in 0..requested_streams {
                // TODO: How to decide which one of the listeners to use? The fuzzer most likely needs to factor
                //       in this knowledge into its fuzzing strategy, i.e. it needs to keep track of streams?
                //       For now we just pick the first one we find.
                let listener_id = *tcp
                    .sockets()
                    .find(|(_, sock)| sock.is_listener())
                    .unwrap()
                    .0;
                tcp.new_connection(&listener_id, SocketAddress::IP(IPAddress::V4 { port: 1337, addr: 42 }))
                    .unwrap();
            }
            // Apply all the inputs, skipping over empty streams
            for (input_data_id, socket_id) in testcase
                .input
                .tcp_streams
                .iter()
                .zip(self.input_info.tcp.streams_writable.iter())
            {
                if let Some(data_id) = input_data_id {
                    let data = self.state.input_data.get(*data_id).unwrap();
                    let mut sock = tcp.socket_mut(*socket_id).unwrap();
                    sock.push_data(&data.data).unwrap();
                }
            }
            // TODO: Run emulation
            // TODO: Gather observations
            // TODO: Store state specific reducer states?
            // TODO: Reset observation channels
            // TODO: Restore emulator snapshot
            testcase.executions += 1;
        }
    }

    fn current_state_id(&self) -> NodeIndex {
        *self.state.state_path.last().unwrap()
    }

    fn fuzz(&mut self) {
        self.clear_observers();
        // Select state
        // Select input from state corpus
        // Do mutations
        // Execute the target with the input(s)
        //
        self.gather_observations();
        sleep(Duration::from_millis(100));
        debug!("Current fuzzer state: {:?}", self.state);
    }

    fn clear_observers(&mut self) {
        for observer in self.observers.iter_mut() {
            observer.borrow_mut().clear();
        }
    }

    fn gather_observations(&mut self) {
        for observer in self.observers.iter_mut() {
            observer.borrow_mut().gather();
        }
    }

    fn reduce_is_interesting(&mut self) -> bool {
        self.is_interesting_reducers
            .iter_mut()
            .map(|r| r.borrow_mut().reduce().unwrap())
            .reduce(|acc, interesting| acc | interesting)
            .unwrap()
    }
}
