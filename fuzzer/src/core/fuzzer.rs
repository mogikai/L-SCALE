use std::cell::RefCell;
use std::collections::{
    BTreeMap,
    HashSet,
};
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::fs;
use std::fs::{
    create_dir_all,
    File,
};
use std::hash::{
    Hash,
    Hasher,
};
use std::io::{
    stdin,
    Read,
    Write,
};
use std::mem::take;
use std::ops::Not;
use std::path::PathBuf;
use std::process::exit;
use std::rc::Rc;
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use petgraph::dot::{
    Config,
    Dot,
};
use petgraph::graph::{
    DiGraph,
    EdgeReference,
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
use tracing::{
    debug,
    trace,
};

use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
use fuzzyemu::emulator::abstractions::mmu::MemoryError;
use fuzzyemu::emulator::abstractions::network::{
    IPAddress,
    SocketAddress,
    SocketId,
};
use fuzzyemu::emulator::abstractions::system::SystemControl;
use fuzzyemu::emulator::interface::emulator::{
    Emulator,
    ExitReason,
};
use fuzzyemu::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};
use fuzzyemu::emulator::platform::linux::kernel::task_id::TaskId;
use fuzzyemu::emulator::system::memory::MappingType;

use crate::api::observer::Observer;
use crate::api::reducer::Reducer;
use crate::observers::emulator::edge::{
    EdgeHitCountObserver,
    EdgeHitObserver,
};
use crate::reducers::edge::behavior::EdgeBehaviorReducer;
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum InputDataSource {
    File(PathBuf),
    #[default]
    Unknown,
}

impl Display for InputDataSource {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            InputDataSource::File(path) => f.write_fmt(format_args!("{}", path.file_name().unwrap().to_string_lossy())),
            InputDataSource::Unknown => f.write_str("Unknown"),
        }
    }
}

/// An object that contains arbitrary bytes that can be used in any kind of input component.
#[derive(Debug, Default, Serialize, Deserialize)]
struct InputData {
    source: InputDataSource,
    data: Vec<u8>,
}

impl From<PathBuf> for InputData {
    fn from(file: PathBuf) -> Self {
        let data = fs::read(file.as_path()).unwrap();
        Self {
            source: InputDataSource::File(file),
            data,
        }
    }
}

new_key_type! { struct InputDataId; }

/// The behavior of a testcase in a specific state.
/// The behavior for now is just a hash of the edge
/// observation for the testcase in the associated state
#[derive(Debug, Serialize, Deserialize)]
pub struct SuffixBehavior {
    edge_map_hash: u64,
}

impl Display for SuffixBehavior {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!("{:>20}", self.edge_map_hash))
    }
}

impl SuffixBehavior {
    pub fn new(edge_map_hash: u64) -> Self {
        Self { edge_map_hash }
    }
}

impl Eq for SuffixBehavior {}

impl PartialEq for SuffixBehavior {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.edge_map_hash.eq(&other.edge_map_hash)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StateType<Type> {
    state: State,
    r#type: Type,
}

#[derive(Debug, Serialize, Deserialize)]
struct State {
    /// The node index in the graph data structure. If the state is a candidate state, this will be None.
    node_id: Option<NodeIndex>,
    label: Option<String>,
    /// How often the target was executed in this state.
    executions: usize,
    /// Describes the behavior of all test case suffixes in this specific state
    suffix_behaviors: SecondaryMap<SuffixId, SuffixBehavior>,
    pending_observations: Vec<SuffixId>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Candidate {
    origin_state_id: StateId,
    transition_testcase: TestCaseId,
}

impl Candidate {
    pub fn new(
        origin_state_id: StateId,
        transition_testcase: TestCaseId,
    ) -> Self {
        Self {
            origin_state_id,
            transition_testcase,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Final;

enum StateComparisonResult {
    IncompleteObservations,
    Equal,
    NotEqual,
}

impl StateType<Candidate> {
    pub fn new(
        origin_state_id: StateId,
        transition_testcase: TestCaseId,
    ) -> Self {
        Self {
            state: State {
                node_id: None,
                label: None,
                executions: 0,
                suffix_behaviors: Default::default(),
                pending_observations: Default::default(),
            },
            r#type: Candidate::new(origin_state_id, transition_testcase),
        }
    }

    pub fn new_with_label(
        label: String,
        origin_state_id: StateId,
        transition_testcase: TestCaseId,
    ) -> Self {
        Self {
            state: State {
                node_id: None,
                label: Some(label),
                executions: 0,
                suffix_behaviors: Default::default(),
                pending_observations: Default::default(),
            },
            r#type: Candidate::new(origin_state_id, transition_testcase),
        }
    }

    pub fn origin(&self) -> StateId {
        self.r#type.origin_state_id
    }

    pub fn reached_by(&self) -> TestCaseId {
        self.r#type.transition_testcase
    }

    pub fn into_final(
        self,
        node_id: NodeIndex,
    ) -> StateType<Final> {
        StateType::<Final> {
            state: State {
                node_id: Some(node_id),
                ..self.state
            },
            r#type: Final,
        }
    }
}

impl StateType<Final> {
    pub fn node_id(&self) -> NodeIndex {
        self.state.node_id.unwrap()
    }
}

impl State {
    /// States are equal if their behavior maps match. If one state has less behavior observations
    /// than the other, these observations will first have to be performed before determining whether
    /// are equal. In this case the function will return `IncompleteObservations`
    /// if the states cannot be distinguished without further observations.
    fn compare_observations(
        &self,
        other: &State,
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
                    if !other_behavior.eq(behavior) {
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

    fn add_suffix_behavior(
        &mut self,
        suffix_id: SuffixId,
        behavior: SuffixBehavior,
    ) {
        trace!("Adding new behavior {behavior:?} for suffix {suffix_id:?}");
        self.suffix_behaviors.insert(suffix_id, behavior);
    }

    fn take_pending_observations(&mut self) -> Vec<SuffixId> {
        take(&mut self.pending_observations)
    }

    fn label(&self) -> &str {
        match &self.label {
            None => "",
            Some(label) => label,
        }
    }
}

new_key_type! { struct StateId; }
new_key_type! { struct StateCandidateId; }
new_key_type! { struct SuffixId; }

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum StateUnion {
    Final(StateId),
    Candidate(StateCandidateId),
}

type InputDataMap = SlotMap<InputDataId, InputData>;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FuzzerState {
    last_dot_file_num: usize,
    /// The state graph. Each node corresponds to an index in the prefix closed set S in Angluins L* algorithm.
    state_graph: DiGraph<(), TestCaseId>,
    suffixes: SlotMap<SuffixId, Vec<TestCaseId>>,
    /// A map of node indices to their associated state id.
    states_map: BTreeMap<NodeIndex, StateId>,
    /// All known states and candidate states. Once a state is moved from the candidate set to the sate set, it
    /// will also be added into the corresponding graph structure.
    states: SlotMap<StateId, StateType<Final>>,
    state_candidates: SlotMap<StateCandidateId, StateType<Candidate>>,
    /// The state candidate set S ∪ A. Each existing state has an associated map that represents candidate
    /// states that could be reached by applying the testcase to this existing state.
    /// If any of these candidate states differ from all existing states in their behavior mapping, then it
    /// will need to be added to the state set.
    state_candidate_map: SecondaryMap<StateId, HashSet<StateCandidateId>>,
    /// A map of various input data blobs that are used by inputs. Multiple inputs may
    /// refer to the same input data blob.
    input_data: InputDataMap,
    /// A map of testcases that are associated with states.
    /// By having the map here globally and only storing indices into the map
    /// it is easily possible to iterate across all testcases of all states.
    test_cases: SlotMap<TestCaseId, TestCase>,
    /// The path through the state graph that the fuzzer has currently taken.
    /// The fuzzer can backtrack along this path.
    state_path: Vec<StateUnion>,
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
    GoTo(StateId),
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
        let mut state =
            StateType::<Candidate>::new_with_label("Initial".to_string(), StateId::default(), TestCaseId::default());
        // Insert each testcase as suffix and push them into the pending observations for the initial state.
        fuzzer_state
            .test_cases
            .keys()
            .map(|testcase_id| fuzzer_state.suffixes.insert(vec![testcase_id]))
            .for_each(|suffix_id| state.state.pending_observations.push(suffix_id));
        let state_id = fuzzer_state.state_candidates.insert(state);

        debug!("Added initial state");
        let state_id = self.promote_to_final_state(fuzzer_state, state_id);
        fuzzer_state.state_path.push(StateUnion::Final(state_id));
    }

    fn shortest_path_from_initial(
        &self,
        fuzzer_state: &FuzzerState,
        state_id: StateId,
    ) -> Vec<StatePathComponent> {
        let StateUnion::Final(initial_state) = fuzzer_state.state_path[0] else {
            panic!("The first state in the path must always be the initial state, which is final");
        };
        // FIXME TODO: For now we just backtrack to initial. In the future we should only backtrack as far as needed.
        let mut path = vec![];
        path.push(StatePathComponent::Backtrack(fuzzer_state.state_path.len().checked_sub(1).unwrap()));

        path.extend(
            petgraph::algo::astar(
                &fuzzer_state.state_graph,
                fuzzer_state.states.get(initial_state).unwrap().node_id(),
                |n| n == fuzzer_state.states.get(state_id).unwrap().node_id(),
                |_| 1,
                |_| 0,
            )
            .unwrap()
            .1
            .iter()
            .map(|node_id| StatePathComponent::GoTo(*fuzzer_state.states_map.get(node_id).unwrap()))
            .skip(1), // Skip initial state
        );

        path
    }

    /// Adds a newly discovered testcase to the testcase set and also creates new candidate states
    /// based on the test case.
    fn add_new_testcase(
        &self,
        fuzzer_state: &mut FuzzerState,
        test_case: TestCase,
    ) {
        let StateUnion::Final(current_state_id) = self.current_state_id(fuzzer_state) else {
            panic!("Can only add new testcases when  in a final state");
        };
        let test_case_id = fuzzer_state.test_cases.insert(test_case);
        for &state in fuzzer_state.states_map.values() {
            // Add a new candidate state
            let state_candidate_id = fuzzer_state
                .state_candidates
                .insert(StateType::<Candidate>::new(current_state_id, test_case_id));
            // and insert it into the candidate set for state `state`.
            fuzzer_state
                .state_candidate_map
                .get_mut(state)
                .unwrap()
                .insert(state_candidate_id);
        }
    }

    ///
    fn current_state_id(
        &self,
        fuzzer_state: &FuzzerState,
    ) -> StateUnion {
        *fuzzer_state.state_path.last().unwrap()
    }

    fn promote_to_final_state(
        &self,
        fuzzer_state: &mut FuzzerState,
        candidate_id: StateCandidateId,
    ) -> StateId {
        let state_candidate = fuzzer_state.state_candidates.remove(candidate_id).unwrap();
        let origin_id = state_candidate.origin();
        let reached_by = state_candidate.reached_by();
        // Remove the candidate form the origins candidate set
        if let Some(candidates) = fuzzer_state.state_candidate_map.get_mut(origin_id) {
            let removed = candidates.remove(&candidate_id);
            debug_assert!(removed);
        }

        // and add a node to the graph and then associate it with the state_id
        let node_id = fuzzer_state.state_graph.add_node(());
        let state_id = fuzzer_state
            .states
            .insert(state_candidate.into_final(node_id));
        fuzzer_state.states_map.insert(node_id, state_id);
        debug!("Promoting state candidate {candidate_id:?} to final state {state_id:?}");

        // If this is the initial state we won't find any origin state.
        let origin_state = fuzzer_state.states.get(origin_id);
        if let Some(origin_state) = origin_state {
            // Add corresponding edge to graph
            fuzzer_state
                .state_graph
                .add_edge(origin_state.node_id(), node_id, reached_by);
        }

        // Insert new candidates for each testcase for the new state
        let old = fuzzer_state
            .state_candidate_map
            .insert(state_id, Default::default());
        debug_assert!(old.is_none());
        let candidate_map = fuzzer_state.state_candidate_map.get_mut(state_id).unwrap();
        for testcase_id in fuzzer_state.test_cases.keys() {
            let mut candidate = StateType::<Candidate>::new(state_id, testcase_id);
            // Add the existing suffixes to the pending observation list such that we can perform them later
            candidate
                .state
                .pending_observations
                .extend(fuzzer_state.suffixes.keys());
            let new_candidate_id = fuzzer_state.state_candidates.insert(candidate);
            candidate_map.insert(new_candidate_id);
            debug!("Adding new state candidate for transition <{state_id:?}--{testcase_id:?}->{new_candidate_id:?}>")
        }
        state_id
    }

    fn find_states_with_missing_observations(
        &self,
        fuzzer_state: &FuzzerState,
    ) -> (Vec<StateId>, HashSet<StateCandidateId>) {
        (
            fuzzer_state
                .states
                .iter()
                .filter_map(|(k, v)| v.state.pending_observations.is_empty().not().then_some(k))
                .collect(),
            fuzzer_state
                .state_candidates
                .iter()
                .filter_map(|(k, v)| v.state.pending_observations.is_empty().not().then_some(k))
                .collect(),
        )
    }

    fn fix_closed_property(
        &self,
        fuzzer_state: &mut FuzzerState,
    ) {
        // As long as we can find violations we need to fix them.
        while let Some(state_candidate_id) = self.find_closed_property_violation(fuzzer_state) {
            trace!("Found closed property violation for state candidate {state_candidate_id:?}");
            self.promote_to_final_state(fuzzer_state, state_candidate_id);
        }
    }

    /// Checks the current candidates against the existing states to find possible closed property
    /// violations. If a candidate is found that violates the closed property, its state candidate id
    /// is returned.
    /// If no violation was found, None is returned.
    /// Runtime: O(n * m) where n = #States and m = #Candidates
    fn find_closed_property_violation(
        &self,
        fuzzer_state: &FuzzerState,
    ) -> Option<StateCandidateId> {
        for candidates in fuzzer_state.state_candidate_map.values() {
            for &candidate_id in candidates {
                if self.candidate_violates_closed_property(fuzzer_state, candidate_id) {
                    // The state set is not closed. Return the origin node, and test case id that can be used
                    // to reach the candidate state.
                    return Some(candidate_id);
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
                state.state.compare_observations(&candidate_state.state),
                StateComparisonResult::Equal | StateComparisonResult::IncompleteObservations
            ) {
                closed = true;
                break;
            }
        }
        !closed
    }

    fn dump_dot_file(
        &self,
        fuzzer_state: &mut FuzzerState,
    ) {
        fuzzer_state.last_dot_file_num += 1;
        let mut states_dir = PathBuf::from(".output/states/");
        create_dir_all(&states_dir).unwrap();
        let edge_labeling = |_, edge: EdgeReference<TestCaseId>| {
            let tc = fuzzer_state.test_cases.get(*edge.weight()).unwrap();
            let streams = tc
                .input
                .tcp_streams
                .iter()
                .fold(String::new(), |acc, stream| {
                    acc + ","
                        + fuzzer_state
                            .input_data
                            .get(stream.unwrap())
                            .unwrap()
                            .source
                            .to_string()
                            .as_str()
                });
            format!("label = \"{:?} | {streams}\"", tc.input.component_markers)
        };
        states_dir.push(fuzzer_state.last_dot_file_num.to_string());
        let dot = Dot::with_attr_getters(
            &fuzzer_state.state_graph,
            &[Config::NodeIndexLabel, Config::EdgeNoLabel],
            &edge_labeling,
            &|_, _| String::new(),
        );
        let mut file = File::options()
            .write(true)
            .create(true)
            .open(states_dir)
            .unwrap();
        file.write_fmt(format_args!("{:?}", dot)).unwrap();
    }

    fn dump_observation_table(
        &self,
        fuzzer_state: &FuzzerState,
    ) {
        for state in fuzzer_state.states.values() {
            let mut line = String::new();
            line += &*format!("{:>40}", format!("{:?}", state.node_id()));
            let mut behaviors = state
                .state
                .suffix_behaviors
                .iter()
                .map(|(id, behavior)| (id, format!("{}", behavior)))
                .collect::<Vec<_>>();
            behaviors.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, behavior) in behaviors {
                line += &*format!(" | {behavior}");
            }
            println!("{line}");
        }
        for state in fuzzer_state.state_candidates.values() {
            let mut line = String::new();
            line += &*format!("{:>40}", format!("{:?}->{:?}", state.origin(), state.reached_by()));
            let mut behaviors = state
                .state
                .suffix_behaviors
                .iter()
                .map(|(id, behavior)| (id, format!("{}", behavior)))
                .collect::<Vec<_>>();
            behaviors.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, behavior) in behaviors {
                line += &*format!(" | {behavior}");
            }
            println!("{line}");
        }
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

#[derive(Debug)]
pub struct Fuzzer<EmulatorT> {
    state: FuzzerState,
    state_builder: StateBuilder,
    observers: Vec<Rc<RefCell<dyn DebuggableObserver>>>,
    is_interesting_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    suffix_behavior_observer: Rc<RefCell<dyn DebuggableObserver>>,
    suffix_behavior_reducer: Rc<RefCell<dyn DebuggableReducer<Output = SuffixBehavior>>>,
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
        let edge_observer = Rc::new(RefCell::new(EdgeHitCountObserver::new(emulator.system().cpu())));
        let edge_reducer = Rc::new(RefCell::new(InterestingEdgesReducer::new(edge_observer.clone())));
        let behavior_observer = Rc::new(RefCell::new(EdgeHitObserver::new(emulator.system().cpu())));
        let behavior_reducer = Rc::new(RefCell::new(EdgeBehaviorReducer::new(behavior_observer.clone())));
        Self {
            state: Default::default(),
            state_builder: Default::default(),
            observers: vec![edge_observer],
            suffix_behavior_observer: behavior_observer,
            is_interesting_reducers: vec![edge_reducer],
            suffix_behavior_reducer: behavior_reducer,
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
        // Do setup stuff
        // Run emulator without inputs until an input is requested.
        let res = self.emulator.run(None, None).unwrap();
        let task_id = TaskId { pid: 1337, tid: 1 };
        self.emulator
            .system()
            .memory()
            .borrow_mut()
            .mappings(&task_id)
            .try_for_each(|mapping| {
                debug!("Mapping: {}", mapping);
                Ok::<(), MemoryError>(())
            })
            .unwrap();
        // We make a snapshot now, so that we can reset to the first input request.
        self.emulator.take_snapshot().unwrap();
        // Reduce once, so that any state of the reducers can be filled with the initial observations.
        assert!(matches!(res, ExitReason::Yielded));
        Self::gather_observations(&mut self.observers);
        Self::reduce_is_interesting(&mut self.is_interesting_reducers);
        Self::clear_observers(&mut self.observers);
        Self::update_input_info(&self.emulator, &mut self.input_info);
        self.load_initial_state();
        while self.evaluate_states_with_missing_observations() {
            self.state_builder.fix_closed_property(&mut self.state);
            self.state_builder.dump_dot_file(&mut self.state);
            self.state_builder.dump_observation_table(&self.state);
            // let mut input = String::new();
            // stdin().read_line(&mut input).unwrap();
            // if input.starts_with('q') {
            //     exit(0);
            // }
        }
    }

    fn load_initial_state(&mut self) {
        self.create_component_marker_inputs();
        let mut corpus_dir = self.context_path.clone();
        corpus_dir.push("corpus");
        for file in corpus_dir.read_dir().unwrap().flatten() {
            if file.file_type().unwrap().is_file() {
                let input_data_id = self.state.input_data.insert(InputData::from(file.path()));
                // TODO: Refactor this into more generic code and don't access private fields.
                let mut input = Input::new();
                input.tcp_streams.push(Some(input_data_id));
                let testcase = TestCase::new(input);
                self.state.test_cases.insert(testcase);
            }
        }
        self.state_builder.initialize_state_info(&mut self.state);
    }

    /// Returns true if observations were performed.
    /// If nothing had to be done, false is returned.
    fn evaluate_states_with_missing_observations(&mut self) -> bool {
        let (states, mut state_candidates) = self
            .state_builder
            .find_states_with_missing_observations(&self.state);
        if states.is_empty() && state_candidates.is_empty() {
            return false;
        }
        trace!("Evaluating states with missing observations: {states:?}, {state_candidates:?}");
        for state_id in states {
            // go to state state_id
            self.go_to_state(state_id);
            self.evaluate_current_state();
            let candidates = self
                .state
                .state_candidate_map
                .get(state_id)
                .unwrap()
                .iter()
                .filter_map(|c| state_candidates.take(c))
                .collect::<Vec<_>>();
            for candidate_id in candidates {
                self.go_to_candidate_from_current(candidate_id);
                self.evaluate_current_state();
                self.go_to_previous_state();
            }
        }
        // We collect all origins for the candidates such that we can visit
        // all the candidates at once for each origin state
        let origin_states = state_candidates
            .iter()
            .map(|&candidate_id| {
                self.state
                    .state_candidates
                    .get(candidate_id)
                    .unwrap()
                    .origin()
            })
            .collect::<HashSet<_>>();
        for origin_state_id in origin_states {
            self.go_to_state(origin_state_id);
            let candidates = self
                .state
                .state_candidate_map
                .get(origin_state_id)
                .unwrap()
                .iter()
                .filter_map(|c| state_candidates.take(c))
                .collect::<Vec<_>>();
            for candidate_id in candidates {
                self.go_to_candidate_from_current(candidate_id);
                self.evaluate_current_state();
                self.go_to_previous_state();
            }
        }

        true
    }

    /// Move the emulator to the specified state
    fn go_to_state(
        &mut self,
        state_id: StateId,
    ) {
        trace!("Traversing to state {state_id:?}");
        if StateUnion::Final(state_id) != *self.state.state_path.last().unwrap() {
            let path = self
                .state_builder
                .shortest_path_from_initial(&self.state, state_id);
            for component in path {
                match component {
                    StatePathComponent::Backtrack(n) => (0..n).for_each(|_| self.go_to_previous_state()),
                    StatePathComponent::GoTo(state_id) => self.go_to_state_from_current(state_id),
                }
            }
        }
        // Nothing to do if we are already in the requested state
    }

    /// Goes to the candidate state if it is directly adjacent to the current state
    fn go_to_candidate_from_current(
        &mut self,
        state_candidate_id: StateCandidateId,
    ) {
        trace!("Moving to candidate {state_candidate_id:?}");
        let candidate = self.state.state_candidates.get(state_candidate_id).unwrap();
        let StateUnion::Final(current_state_id) = self.state_builder.current_state_id(&self.state) else {
            panic!("Candidates can only be visited from a final state");
        };
        assert_eq!(candidate.origin(), current_state_id, "The candidate must be reachable from the current state");

        let testcase = self
            .state
            .test_cases
            .get_mut(candidate.reached_by())
            .unwrap();
        Self::apply_testcase(&mut self.emulator, &mut self.input_info, &self.state.input_data, testcase);
        self.emulator.take_snapshot().unwrap();
        self.state
            .state_path
            .push(StateUnion::Candidate(state_candidate_id));
    }

    fn go_to_state_from_current(
        &mut self,
        state_id: StateId,
    ) {
        trace!("Moving to state {state_id:?}");
        let target_state = self.state.states.get(state_id).unwrap();
        let StateUnion::Final(current_state_id) = self.state_builder.current_state_id(&self.state) else {
            panic!("States can only be visited from a final state");
        };
        let current_state = self.state.states.get(current_state_id).unwrap();
        let mut edges = self
            .state
            .state_graph
            .edges_connecting(current_state.node_id(), target_state.node_id());
        let reached_by = edges.next().unwrap().weight();
        assert_eq!(edges.next(), None);

        let testcase = self.state.test_cases.get_mut(*reached_by).unwrap();
        Self::apply_testcase(&mut self.emulator, &mut self.input_info, &self.state.input_data, testcase);
        self.emulator.take_snapshot().unwrap();
        self.state.state_path.push(StateUnion::Final(state_id));
    }

    fn go_to_previous_state(&mut self) {
        if self.state.state_path.len() <= 1 {
            panic!("Cannot go back past the initial state");
        }
        trace!("Moving to previous state");
        self.emulator
            .discard_current_and_restore_previous_snapshot()
            .unwrap();
        self.state.state_path.pop();
    }

    fn create_component_marker_inputs(&mut self) {
        // TODO: Add an individual testcase for each listener socket as soon as we can identify the sockets.
        if !self.input_info.tcp.listeners.is_empty() {
            let mut input = Input::new();
            input.component_markers.push(ComponentMarker::NewTcpStream);
            let testcase = TestCase::new(input);
            self.state.test_cases.insert(testcase);
        }
    }

    // TODO :Refactor this into the emulator such that we can just get it from there. No need to store externally.
    fn update_input_info(
        emulator: &EmulatorT,
        input_info: &mut EmulatorInputInfo,
    ) {
        input_info.tcp.listeners.clear();
        input_info.tcp.streams_writable.clear();
        for (&sock_id, sock) in emulator.system().tcp().borrow().sockets() {
            trace!("File operations ready for socket {sock_id:?}: {:?}", sock.interest());
            if sock
                .interest()
                .intersects(FileOperationsReady::Read | FileOperationsReady::ReadNormal)
            {
                if sock.is_listener() {
                    input_info.tcp.listeners.push(sock_id);
                } else {
                    input_info.tcp.streams_writable.push(sock_id);
                }
            }
        }
    }

    /// Execute all corpus entries in the state and gather observations for each entry.
    /// There might occur state conflicts. These will be saved and can later be used
    /// to enhance the state machine based on these conflicts.
    fn evaluate_current_state(&mut self) {
        let state = {
            let state_id = self.state.state_path.last().unwrap();
            match state_id {
                StateUnion::Final(state_id) => &mut self.state.states.get_mut(*state_id).unwrap().state,
                StateUnion::Candidate(state_id) => {
                    &mut self
                        .state
                        .state_candidates
                        .get_mut(*state_id)
                        .unwrap()
                        .state
                }
            }
        };
        trace!("Evaluating current state {state:?}");

        for suffix_id in state.take_pending_observations() {
            self.suffix_behavior_observer.borrow_mut().clear();
            trace!("Processing suffix {suffix_id:?}");
            let suffix = self.state.suffixes.get(suffix_id).unwrap();
            // Apply each testcase in the suffix to the target.
            for testcase_id in suffix {
                // TODO: Store state specific reducer states?
                Self::clear_observers(&mut self.observers);
                let testcase = self.state.test_cases.get_mut(*testcase_id).unwrap();
                Self::apply_testcase(&mut self.emulator, &mut self.input_info, &self.state.input_data, testcase);
                Self::gather_observations(&mut self.observers);
                if Self::reduce_is_interesting(&mut self.is_interesting_reducers) {
                    debug!("THIS INTERESTING!!");
                }
            }
            self.suffix_behavior_observer.borrow_mut().gather();
            let behavior = self.suffix_behavior_reducer.borrow_mut().reduce().unwrap();
            state.add_suffix_behavior(suffix_id, behavior);

            self.emulator.restore_snapshot().unwrap();
            state.executions += 1;
        }
    }

    /// Runs the emulator with the provided testcase. The emulator will not perform any
    /// snapshot or restore operations.
    fn apply_testcase(
        emulator: &mut EmulatorT,
        input_info: &mut EmulatorInputInfo,
        input_data_map: &InputDataMap,
        testcase: &mut TestCase,
    ) {
        trace!("Applying testcase {testcase:?}");
        Self::update_input_info(emulator, input_info);
        let mut tcp = emulator.system().tcp().borrow_mut();
        // TODO: Make this configurable or adjustable by the fuzzer. For now only create streams if none are already available
        if input_info.tcp.streams_writable.is_empty() {
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
        }
        // Apply all the inputs, skipping over empty streams
        for (input_data_id, socket_id) in testcase
            .input
            .tcp_streams
            .iter()
            .zip(input_info.tcp.streams_writable.iter())
        {
            if let Some(data_id) = input_data_id {
                let data = input_data_map.get(*data_id).unwrap();
                let sock = tcp.socket_mut(*socket_id).unwrap();
                trace!("Applying data from {}", data.source);
                let _ = sock.push_data(&data.data);
            }
        }
        drop(tcp);
        // TODO: Wake up the correct tasks. Might make sense to put this feature into the fuzzer and that
        // TODO: The system components register the wake up request in some list?
        emulator
            .system()
            .task_manager()
            .borrow_mut()
            .tasks_mut()
            .for_each(|task| task.wake_up());
        let status = emulator.run(None, None).unwrap();
        assert!(matches!(status, ExitReason::Yielded));
        testcase.executions += 1;
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

    fn clear_observers(observers: &mut [Rc<RefCell<dyn DebuggableObserver>>]) {
        for observer in observers.iter_mut() {
            observer.borrow_mut().clear();
        }
    }

    fn gather_observations(observers: &mut [Rc<RefCell<dyn DebuggableObserver>>]) {
        for observer in observers.iter_mut() {
            observer.borrow_mut().gather();
        }
    }

    fn reduce_is_interesting(reducers: &mut [Rc<RefCell<dyn DebuggableReducer<Output = bool>>>]) -> bool {
        reducers
            .iter_mut()
            .map(|r| r.borrow_mut().reduce().unwrap())
            .reduce(|acc, interesting| acc | interesting)
            .unwrap()
    }
}
