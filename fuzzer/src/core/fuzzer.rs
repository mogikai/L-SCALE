use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::hash::{
    Hash,
    Hasher,
};
use std::marker::PhantomData;
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
    SecondaryMap,
    SlotMap,
};

use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
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
use crate::observers::emulator::edge::EdgeObserver;
use crate::reducers::edge::InterestingEdgesReducer;

/// Testcases have an associated input and some metadata.
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

new_key_type! { struct TestCaseId; }

#[derive(Debug, Serialize, Deserialize)]
enum ComponentMarker {
    // TODO: Add some kind of unique identifier for the tcp stream. Cannot use the SocketId since it is not
    // TODO: unique across different state paths. Might be possible to use the socket address or something
    // TODO: similar. For example a tcp listener could be identified via the Sockaddr
    NewTcpStream,
}

/// An emulator input. The input may have multiple streams for each input component of the emulator.
/// The fuzzer will prepare the inputs such that the number of streams matches the amount
/// of streams that are currently awaiting input in the emulator.
/// The mutators can then decide whether or not to fulfil these demands.
/// At least one stream has to contain input, otherwise the emulation will be a no-op.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Input {
    /// A list of component markers that signal to the fuzzer that a component action
    /// other than read or write should be performed, e.g. creating a new tcp stream.
    component_markers: Vec<ComponentMarker>,
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
#[derive(Debug, Default, Serialize, Deserialize)]
struct InputData {
    data: Vec<u8>,
}

impl From<Vec<u8>> for InputData {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

new_key_type! { struct InputDataId; }

/// The behavior of a testcase in a specific state.
/// The behavior for now is just a hash of the edge
/// observation for the testcase in the associated state
#[derive(Debug, Default, Serialize, Deserialize)]
struct TestCaseBehavior {
    /// The path on which this behavior was observed.
    /// If we find a new path to the same state (and correspondingly
    /// this testcases behavior), and observe a different behavior,
    /// then we need to create a new state for the testcase.
    discovery_path: Vec<NodeIndex>,
    edge_map_hash: usize,
}

impl Eq for TestCaseBehavior {}

impl PartialEq for TestCaseBehavior {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.edge_map_hash.eq(&other.edge_map_hash)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct State<Type> {
    /// The node index in the graph data structure. If the state is a candidate state, this will be None.
    node_id: Option<NodeIndex>,
    label: Option<String>,
    /// How often the target was executed in this state.
    executions: usize,
    /// Describes the behavior of all test case suffixes in this specific state
    suffix_behaviors: SecondaryMap<SuffixId, TestCaseBehavior>,
    pending_observations: Vec<SuffixId>,
    _phantom: PhantomData<Type>,
}

#[derive(Debug)]
struct Candidate;
#[derive(Debug)]
struct Final;

enum StateComparisonResult {
    IncompleteObservations,
    Equal,
    NotEqual,
}

impl State<Candidate> {
    pub fn new() -> Self {
        Self {
            node_id: None,
            label: None,
            executions: 0,
            suffix_behaviors: Default::default(),
            pending_observations: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn new_with_label(label: String) -> Self {
        Self {
            node_id: None,
            label: Some(label),
            executions: 0,
            suffix_behaviors: Default::default(),
            pending_observations: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn into_final(
        self,
        node_id: NodeIndex,
    ) -> State<Final> {
        State::<Final> {
            node_id: Some(node_id),
            label: self.label,
            executions: self.executions,
            suffix_behaviors: self.suffix_behaviors,
            pending_observations: Default::default(),
            _phantom: Default::default(),
        }
    }
}

impl State<Final> {
    pub fn new(node_id: NodeIndex) -> Self {
        Self {
            node_id: Some(node_id),
            label: None,
            executions: 0,
            suffix_behaviors: Default::default(),
            pending_observations: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn new_with_label(
        node_id: NodeIndex,
        label: String,
    ) -> Self {
        Self {
            node_id: Some(node_id),
            label: Some(label),
            executions: 0,
            suffix_behaviors: Default::default(),
            pending_observations: Default::default(),
            _phantom: Default::default(),
        }
    }

    pub fn node_id(&self) -> NodeIndex {
        self.node_id.unwrap()
    }
}

impl<Type> State<Type> {
    /// States are equal if their behavior maps match. If one state has less behavior observations
    /// than the other, these observations will first have to be performed before determining whether
    /// are equal. In this case the function will return `IncompleteObservations`
    /// if the states cannot be distinguished without further observations.
    fn equal<OtherType>(
        &self,
        other: &State<OtherType>,
    ) -> StateComparisonResult {
        let mut result = if self.suffix_behaviors.len() == other.suffix_behaviors.len() {
            StateComparisonResult::Equal
        } else {
            StateComparisonResult::IncompleteObservations
        };
        for (suffix_id, behavior) in &self.suffix_behaviors {
            match other.suffix_behaviors.get(suffix_id) {
                // The table is missing an observation which we still have to get from the target
                // Keep looking for differing entries since we can then definitely say that the
                // states differ.
                None => result = StateComparisonResult::IncompleteObservations,
                Some(other_behavior) => {
                    if !other_behavior.eq(&behavior) {
                        // The behaviors differ, hence the states are not equal.
                        // It is ok to shortcut here, since even if there are some
                        // observations missing we can already say that the sates will
                        // be different since they only have to differ in at least one observation.
                        return StateComparisonResult::NotEqual;
                    }
                }
            }
        }
        result
    }
}

new_key_type! { struct StateId; }
new_key_type! { struct StateCandidateId; }
new_key_type! { struct SuffixId; }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FuzzerState {
    /// The state graph. Each node corresponds to an index in the prefix closed set S in Angluins L* algorithm.
    state_graph: DiGraph<(), ()>,
    suffixes: SlotMap<SuffixId, Vec<TestCaseId>>,
    /// A map of node indices to their associated state id.
    states_map: BTreeMap<NodeIndex, StateId>,
    /// All known states and candidate states. Once a state is moved from the candidate set to the sate set, it
    /// will also be added into the corresponding graph structure.
    states: SlotMap<StateId, State<Final>>,
    state_candidates: SlotMap<StateCandidateId, State<Candidate>>,
    /// The state candidate set S ∪ A. Each existing state has an associated map that represents candidate
    /// states that could be reached by applying the testcase to this existing state.
    /// If any of these candidate states differ from all existing states in their behavior mapping, then it
    /// will need to be added to the state set.
    state_candidate_map: SecondaryMap<StateId, SecondaryMap<TestCaseId, StateCandidateId>>,
    /// A map of various input data blobs that are used by inputs. Multiple inputs may
    /// refer to the same input data blob.
    input_data: SlotMap<InputDataId, InputData>,
    /// A map of testcases that are associated with states.
    /// By having the map here globally and only storing indices into the map
    /// it is easily possible to iterate across all testcases of all states.
    test_cases: SlotMap<TestCaseId, TestCase>,
    /// The path through the state graph that the fuzzer has currently taken.
    /// The fuzzer can backtrack along this path.
    state_path: Vec<NodeIndex>,
}

/// A path component that the state manager will supply when asked for a path to a state.
/// The state originates in the initial state. Since the fuzzer might be on any state
/// path currently, the path to the new state will likely contain some backtracking
/// operations until the node is reached where the path diverges from the already taken
/// path.
pub enum StatePathComponent {
    /// The backtrack variant signals the fuzzer to restore an older state. The number
    /// in the variant signals how many times the backtracking has to occur.
    Backtrack(usize),
    Testcase(TestCaseId),
}

#[derive(Debug, Default)]
struct StateBuilder;

impl StateBuilder {
    fn new() -> Self {
        Self {}
    }

    /// Initializes the state info in the fuzzer state.
    /// Creates an initial state and sets up all the associated data structures
    fn initialize_state_info(
        &self,
        fuzzer_state: &mut FuzzerState,
    ) {
        let state_id = fuzzer_state
            .state_candidates
            .insert(State::<Candidate>::new_with_label("Initial".to_string()));
        debug!("Added initial state");
        self.promote_to_final_state(fuzzer_state, state_id);
    }

    fn add_behavior_to_current_state(
        &self,
        fuzzer_state: &mut FuzzerState,
        behavior: TestCaseBehavior,
    ) {
    }

    fn shortest_path_from_initial(
        &self,
        fuzzer_state: FuzzerState,
    ) -> Vec<StatePathComponent> {
        todo!()
    }

    /// Adds a newly discovered testcase to the testcase set and also creates new candidate states
    /// based on the test case.
    fn add_new_testcase(
        &self,
        fuzzer_state: &mut FuzzerState,
        test_case: TestCase,
    ) {
        let test_case_id = fuzzer_state.test_cases.insert(test_case);
        for &state in fuzzer_state.states_map.values() {
            // Add a new candidate state
            let state_candidate_id = fuzzer_state
                .state_candidates
                .insert(State::<Candidate>::new());
            // and insert it into the candidate set for state `state`.
            fuzzer_state
                .state_candidate_map
                .get_mut(state)
                .unwrap()
                .insert(test_case_id, state_candidate_id);
        }
    }

    fn promote_to_final_state(
        &self,
        fuzzer_state: &mut FuzzerState,
        candidate_id: StateCandidateId,
    ) -> NodeIndex {
        let node_id = fuzzer_state.state_graph.add_node(());
        // and add a node to the graph and then associate it with the state_id
        let state_id = fuzzer_state.states.insert(
            fuzzer_state
                .state_candidates
                .remove(candidate_id)
                .unwrap()
                .into_final(node_id),
        );
        debug!("Promoting state candidate {candidate_id:?} to final state {state_id:?}");
        fuzzer_state.states_map.insert(node_id, state_id);
        let old = fuzzer_state
            .state_candidate_map
            .insert(state_id, Default::default());
        debug_assert!(old.is_none());
        let candidate_map = fuzzer_state.state_candidate_map.get_mut(state_id).unwrap();
        for testcase_id in fuzzer_state.test_cases.keys() {
            let new_candidate_id = fuzzer_state
                .state_candidates
                .insert(State::<Candidate>::new());
            candidate_map.insert(testcase_id, new_candidate_id);
            debug!("Adding new state candidate for transition <{state_id:?}--{testcase_id:?}->{new_candidate_id:?}>")
        }
        node_id
    }

    fn fix_closed_property(
        &self,
        fuzzer_state: &mut FuzzerState,
    ) {
        // As long as we can find violations we need to fix them.
        while let Some((origin_id, test_case_id)) = self.find_closed_property_violation(fuzzer_state) {
            // Remove from candidates
            let candidate_id = fuzzer_state
                .state_candidate_map
                .get_mut(origin_id)
                .unwrap()
                .remove(test_case_id)
                .unwrap();
            let state_id = self.promote_to_final_state(fuzzer_state, candidate_id);
            let origin = fuzzer_state.states.get(origin_id).unwrap().node_id();
            fuzzer_state.state_graph.add_edge(origin, state_id, ());
        }
    }

    /// Checks the current candidates against the existing states to find possible closed property
    /// violations. If a candidate is found that violates the closed property, the origin state
    /// and the test_cased_id to reach the candidate are returned.
    /// If no violation was found, None is returned.
    /// Runtime: O(n * m) where n = #States and m = #Candidates
    fn find_closed_property_violation(
        &self,
        fuzzer_state: &FuzzerState,
    ) -> Option<(StateId, TestCaseId)> {
        for (origin_node, candidates) in &fuzzer_state.state_candidate_map {
            for (test_case_id, &candidate) in candidates {
                if self.candidate_violates_closed_property(fuzzer_state, candidate) {
                    // The state set is not closed. Return the origin node, and test case id that can be used
                    // to reach the candidate state.
                    return Some((origin_node, test_case_id));
                }
            }
        }
        None
    }

    fn candidate_violates_closed_property(
        &self,
        fuzzer_state: &FuzzerState,
        candidate_id: StateCandidateId,
    ) -> bool {
        let mut closed = false;
        for &state_id in fuzzer_state.states_map.values() {
            let state = fuzzer_state.states.get(state_id).unwrap();
            let candidate_state = fuzzer_state.state_candidates.get(candidate_id).unwrap();
            // If the states are equal, then the closed property is upheld.
            // Only if all states are not equal to the candidate, the closed
            // property is violated.
            // In case of incomplete observations we treat the states as equal for now,
            // since we can only decide to add the candidate once we observe that it is
            // explicitly not equal to all existing states.
            if matches!(
                state.equal(candidate_state),
                StateComparisonResult::Equal | StateComparisonResult::IncompleteObservations
            ) {
                closed = true;
                break;
            }
        }
        !closed
    }
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
    state_builder: StateBuilder,
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
            state_builder: Default::default(),
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
        // while do_fuzzing.load(Ordering::SeqCst) {
        //     self.fuzz();
        // }
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
        Self::gather_observations(&mut self.observers);
        Self::reduce_is_interesting(&mut self.is_interesting_reducers);
        Self::clear_observers(&mut self.observers);
        self.update_input_info();
        self.load_initial_state();
        self.evaluate_state();
    }

    fn load_initial_state(&mut self) {
        self.create_component_marker_inputs();
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
                    self.state.test_cases.insert(testcase);
                }
            }
        }
        self.state_builder.initialize_state_info(&mut self.state);
    }

    fn create_component_marker_inputs(&mut self) {
        // TODO: Add an individual testcase for each listener socket as soon as we can identify the sockets.
        if self.input_info.tcp.listeners.len() > 0 {
            let mut input = Input::new();
            input.component_markers.push(ComponentMarker::NewTcpStream);
            let testcase = TestCase::new(input);
            self.state.test_cases.insert(testcase);
        }
    }

    fn update_input_info(&mut self) {
        self.input_info.tcp.listeners.clear();
        self.input_info.tcp.streams_writable.clear();
        for (&sock_id, sock) in self.emulator.system().tcp().borrow().sockets() {
            debug!("{:?}", sock.interest());
            if sock
                .interest()
                .intersects(FileOperationsReady::Read | FileOperationsReady::ReadNormal)
            {
                if sock.is_listener() {
                    self.input_info.tcp.listeners.push(sock_id);
                } else {
                    self.input_info.tcp.streams_writable.push(sock_id);
                }
            }
        }
    }

    /// Execute all corpus entries in the state and gather observations for each entry.
    /// There might occur state conflicts. These will be saved and can later be used
    /// to enhance the state machine based on these conflicts.
    fn evaluate_state(&mut self) {
        self.update_input_info();
        for testcase in self.state.test_cases.values_mut() {
            let mut tcp = self.emulator.system().tcp().borrow_mut();
            for marker in &testcase.input.component_markers {
                if let ComponentMarker::NewTcpStream = marker {
                    // TODO: How to decide which one of the listeners to use? The fuzzer most likely needs to factor
                    //       in this knowledge into its fuzzing strategy, i.e. it needs to keep track of streams?
                    //       For now we just pick the first one we find.
                    let listener_id = *tcp
                        .sockets()
                        .find(|(_, sock)| sock.is_listener())
                        .unwrap()
                        .0;
                    tcp.new_connection(
                        &listener_id,
                        SocketAddress::IP(IPAddress::V6 {
                            port: 1337,
                            flowinfo: 0,
                            addr: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                            scope_id: 0,
                        }),
                    )
                    .unwrap();
                }
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
            drop(tcp);
            // TODO: Wake up the correct tasks. Might make sense to put this feature into the fuzzer and that
            // TODO: The system components register the wake up request in some list?
            self.emulator
                .system()
                .task_manager()
                .borrow_mut()
                .tasks_mut()
                .for_each(|task| task.wake_up());
            let status = self.emulator.run(None, None).unwrap();
            assert!(matches!(status, ExitStatus::Yielded));
            Self::gather_observations(&mut self.observers);
            if Self::reduce_is_interesting(&mut self.is_interesting_reducers) {
                debug!("THIS INTERESTING!!");
            }
            // TODO: Store state specific reducer states?
            Self::clear_observers(&mut self.observers);
            self.emulator.restore().unwrap();
            testcase.executions += 1;
            let node_index = self.state.state_path.last().unwrap();
            let current_state = *self.state.states_map.get(node_index).unwrap();
            let current_state = self.state.states.get_mut(current_state).unwrap();
            current_state.executions += 1;
        }
    }

    fn fuzz(&mut self) {
        Self::clear_observers(&mut self.observers);
        // Select state
        // Select input from state corpus
        // Do mutations
        // Execute the target with the input(s)
        //
        Self::gather_observations(&mut self.observers);
        sleep(Duration::from_millis(100));
        debug!("Current fuzzer state: {:?}", self.state);
    }

    fn clear_observers(observers: &mut Vec<Rc<RefCell<dyn DebuggableObserver>>>) {
        for observer in observers.iter_mut() {
            observer.borrow_mut().clear();
        }
    }

    fn gather_observations(observers: &mut Vec<Rc<RefCell<dyn DebuggableObserver>>>) {
        for observer in observers.iter_mut() {
            observer.borrow_mut().gather();
        }
    }

    fn reduce_is_interesting(reducers: &mut Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>) -> bool {
        reducers
            .iter_mut()
            .map(|r| r.borrow_mut().reduce().unwrap())
            .reduce(|acc, interesting| acc | interesting)
            .unwrap()
    }
}
