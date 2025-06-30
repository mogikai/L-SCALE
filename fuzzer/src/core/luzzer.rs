use crate::api::observer::{
    DebuggableObserver,
    Observe,
};
use crate::api::reducer::Reducer;
use crate::observers::new::bb::BasicBlockObserver;
use crate::reducers::bb::lsh::BlockHitLshReducer;
use bon::Builder;
use fnv::FnvHashMap;
use mogi_emulator::emulator::abstractions::file_descriptors::FileOperationsReady;
use mogi_emulator::emulator::abstractions::network::{
    IPAddress,
    SocketAddress,
    SocketId,
};
use mogi_emulator::emulator::abstractions::system::SystemControl;
use mogi_emulator::emulator::interface::emulator::{
    Emulator,
    ExitReason,
};
use mogi_emulator::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};
use mogi_emulator::emulator::platform::linux::kernel::task_id::TaskId;
use mogi_statelearner::row::ObservationRowId;
use mogi_statelearner::{
    ObservationValue,
    StateLearner,
    SymbolId,
};
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::components::mmu::Permission;
use mogi_utilities::error::memory::MemoryError;
use rand::prelude::IndexedRandom;
use rand::{
    Rng,
    SeedableRng,
};
use rand_chacha::ChaCha8Rng;
use serde::{
    Deserialize,
    Serialize,
};
use slotmap::{
    Key,
    SlotMap,
    new_key_type,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::fs;
use std::hash::{
    Hash,
    Hasher,
};
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::time::Instant;
use tlsh::{
    FuzzyHashType,
    hashes,
};
use tracing::{
    debug,
    info,
    trace,
};

new_key_type! { pub struct ObservationId; }
impl Display for ObservationId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:?}", self.data())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Observation {
    edge_hash_map: hashes::Long,
}

impl Hash for Observation {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        state.write(self.edge_hash_map.to_string().as_bytes());
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FuzzyObservation {
    observation: Observation,
    thresold: u8,
}

impl FuzzyObservation {
    pub fn new(
        observation: Observation,
        thresold: u8,
    ) -> Self {
        Self { observation, thresold }
    }
}

impl PartialEq for FuzzyObservation {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.observation
            .edge_hash_map
            .compare(&other.observation.edge_hash_map)
            <= self.thresold as u32
    }
}

impl ObservationValue for FuzzyObservation {}

impl Display for Observation {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.edge_hash_map)
    }
}

impl Observation {
    pub fn new(edge_hash_map: hashes::Long) -> Self {
        Self { edge_hash_map }
    }
}

impl ObservationValue for ObservationId {}

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

new_key_type! { pub struct InputDataId; }

impl Display for InputDataId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{:?}", self.data())
    }
}

#[derive(Debug)]
enum ComponentMarker {
    NewTcpStream,
    #[allow(unused)]
    CloseTcpStream,
}

/// An object that contains arbitrary bytes that can be used in any kind of input component.
#[derive(Debug, Default, Serialize, Deserialize)]
struct InputData {
    source: InputDataSource,
    data: Vec<u8>,
}

impl InputData {
    fn load_from_file(file: &Path) -> Self {
        let data = fs::read(file).unwrap();
        Self {
            source: InputDataSource::File(file.to_path_buf()),
            data,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum InputDataSource {
    File(PathBuf),
    #[default]
    Unknown,
}

#[derive(Debug, Default)]
pub struct TestCase {
    /// The number of times this test case was applied to the target
    executions: usize,
    component_markers: Vec<ComponentMarker>,
    tcp_streams: Vec<InputDataId>,
}

impl Display for TestCase {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "TC(exe: {}, tcp: ", self.executions)?;
        write!(f, "[")?;
        for (i, id) in self.tcp_streams.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{id}")?;
        }
        write!(f, "]")?;
        write!(f, ")")
    }
}

enum SequenceMutation {
    Insert,
    Replace,
    Swap,
}

impl SequenceMutation {
    pub fn random(rng: &mut impl Rng) -> Self {
        match rng.random_range(0..=6) {
            0..5 => Self::Insert,
            5 => Self::Replace,
            6 => Self::Swap,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
pub struct LuzzerState {
    test_cases: SlotMap<SymbolId, TestCase>,
    input_data: SlotMap<InputDataId, InputData>,
    learner: StateLearner<ObservationId>,
    /// The current prefix the learner has taken through the target.
    /// There exists a snapshot of the emulator for each prefix symbol.
    /// If the prefix is popped, the state of the emulator has to be reset.
    current_prefix: Vec<SymbolId>,
    symbols_applied: usize,
    observations: SlotMap<ObservationId, Observation>,
    observation_mapping: HashMap<Observation, ObservationId>,
}

#[derive(Debug, Builder)]
pub struct LuzzerConfig {
    do_refinement: Option<usize>,
    #[builder(default = false)]
    detailed_graph_building: bool,
    context_path: PathBuf,
    executable_path: PathBuf,
    #[builder(default = 0)]
    threshold: u64,
}

#[derive(Debug)]
pub struct Luzzer<EmulatorT> {
    state: LuzzerState,
    observers: Vec<Rc<RefCell<dyn DebuggableObserver>>>,
    is_interesting_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    behavior_observer: Rc<RefCell<dyn DebuggableObserver>>,
    suffix_behavior_reducer: Rc<RefCell<dyn DebuggableReducer<Output = Observation>>>,
    #[allow(dead_code)]
    is_crash_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    emulator: EmulatorT,
    input_info: EmulatorInputInfo,
    start_time: Instant,
    config: LuzzerConfig,
}

impl<EmulatorT> Luzzer<EmulatorT>
where
    EmulatorT: Emulator,
{
    pub fn new(
        emulator: EmulatorT,
        config: LuzzerConfig,
    ) -> Self {
        let memory = emulator.system().memory().borrow();
        let exe_name = config
            .executable_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let exe_mappings = memory
            .mappings(&TaskId { pid: 1337, tid: 1 })
            .filter_map(|mapping| {
                mapping.label.as_ref().and_then(|label| {
                    if label.contains(exe_name) && mapping.permissions.contains(Permission::EXEC) {
                        Some(mapping)
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();
        let start = exe_mappings
            .iter()
            .min_by_key(|m| &m.v_start)
            .unwrap()
            .v_start;
        let end = exe_mappings
            .iter()
            .max_by_key(|m| m.v_end())
            .unwrap()
            .v_end();
        drop(memory);
        // let edge_observer = Rc::new(RefCell::new(EdgeHitCountObserver::new()));
        // let edge_reducer = Rc::new(RefCell::new(InterestingEdgesReducer::new(edge_observer.clone())));
        // let behavior_observer = BlockHitObserver::new();
        let block_observer = Rc::new(RefCell::new(BasicBlockObserver::new(start, end)));
        let behavior_observer = Rc::new(RefCell::new(BasicBlockObserver::new(start, end)));
        let behavior_reducer = Rc::new(RefCell::new(BlockHitLshReducer::new(behavior_observer.clone())));

        {
            let mut cpu = emulator.system().cpu().borrow_mut();
            let hook_block_observer = block_observer.clone();
            let hook_behavior_observer = behavior_observer.clone();
            cpu.add_block_hook(
                start,
                end,
                Box::new(move |_cpu, addr, _| {
                    hook_block_observer.borrow_mut().observe(&addr);
                    hook_behavior_observer.borrow_mut().observe(&addr);
                }),
            )
            .unwrap();
        }

        // let bb_collector = Rc::new(RefCell::new(BasicBlockCollector::new(
        //     emulator.system().cpu(),
        //     start,
        //     end,
        //     [
        //         // &behavior_observer.shared_observe(),
        //         // &edge_observer.borrow().shared_observe(),
        //     ]
        //     .into_iter(),
        // )));
        // emulator.register_meter(bb_collector);
        // let behavior_observer = Rc::new(RefCell::new(behavior_observer));
        // let behavior_reducer = Rc::new(RefCell::new(BasicBlockBehaviorReducer::new(behavior_observer.clone())));

        Self {
            state: LuzzerState::default(),
            observers: vec![block_observer],
            behavior_observer,
            is_interesting_reducers: vec![],
            suffix_behavior_reducer: behavior_reducer,
            is_crash_reducers: Default::default(),
            emulator,
            input_info: Default::default(),
            start_time: Instant::now(),
            config,
        }
    }

    pub fn run(&mut self) {
        // let do_fuzzing = Arc::new(AtomicBool::new(true));
        // {
        //     let do_fuzzing = do_fuzzing.clone();
        //     ctrlc::set_handler(move || {
        //         do_fuzzing.store(false, Ordering::SeqCst);
        //     })
        //     .expect("Error setting handler");
        // }
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        self.setup();
        self.start_time = Instant::now();
        self.build_state_model(0, &mut rng);
        if let Some(count) = self.config.do_refinement {
            for i in 0..count {
                info!("-------------------------------------------------------------------------");
                info!("Doing Sequence mutation phase");
                self.do_sequence_mutations(&mut rng);
                info!("Rebuilding state model after sequence mutations");
                self.build_state_model(0, &mut rng);
            }
            info!("--------------------------------------------------");
            info!("Building final state model.");
            self.build_state_model(0, &mut rng);
        }
        // while do_fuzzing.load(Ordering::SeqCst) {
        self.fuzz();
        // }
    }

    fn do_sequence_mutations(
        &mut self,
        rng: &mut impl Rng,
    ) {
        let mut new_prefixes = vec![];
        let symbols = self.state.test_cases.keys().collect::<Vec<_>>();
        for prefix in self.state.learner.prefixes() {
            if prefix.len() == 0 {
                continue;
            }
            if rng.random_range(0.0..1.0) < 0.6 {
                continue;
            }
            if new_prefixes.len() > 50 {
                break;
            }
            // We picked the prefix. Mutate now.
            // Collect the prefix to be able to mutate it.
            let mut new_prefix = prefix.collect::<Vec<_>>();

            // Do between one and 10 random mutations.
            for _ in 0..rng.random_range(1..=10) {
                match SequenceMutation::random(rng) {
                    SequenceMutation::Insert => {
                        let index = if new_prefix.len() > 1 {
                            rng.random_range(0..new_prefix.len())
                        } else {
                            0
                        };
                        let symbol = *symbols.choose(rng).unwrap();
                        new_prefix.insert(index, symbol);
                    }
                    SequenceMutation::Replace => {
                        let index = if new_prefix.len() > 1 {
                            rng.random_range(0..new_prefix.len())
                        } else {
                            0
                        };
                        let symbol = *symbols.choose(rng).unwrap();
                        new_prefix[index] = symbol;
                    }
                    SequenceMutation::Swap => {
                        if new_prefix.len() > 1 {
                            let index_a = rng.random_range(0..new_prefix.len());
                            let index_b = rng.random_range(0..new_prefix.len());
                            new_prefix.swap(index_a, index_b);
                        }
                    }
                }
            }

            new_prefixes.push(new_prefix);
        }
        new_prefixes.into_iter().for_each(|prefix| {
            self.state.learner.add_prefix(prefix);
        })
    }

    fn setup(&mut self) {
        let task_id = TaskId { pid: 1337, tid: 1 };
        // Do setup stuff
        // Run emulator without inputs until an input is requested.
        let res = self.emulator.run(None, None);
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
        let res = res.unwrap();

        // Reduce once, so that any state of the reducers can be filled with the initial observations.
        assert!(matches!(res, ExitReason::Yielded));
        Self::gather_observations(&mut self.observers);
        Self::reduce_is_interesting(&mut self.is_interesting_reducers);
        Self::clear_observers(&mut self.observers);
        Self::update_input_info(&self.emulator, &mut self.input_info);

        self.load_initial_state();

        let status = self.emulator.run(None, None).unwrap();
        assert!(matches!(status, ExitReason::Yielded));
        Self::gather_observations(&mut self.observers);
        Self::reduce_is_interesting(&mut self.is_interesting_reducers);
        Self::clear_observers(&mut self.observers);
        Self::update_input_info(&self.emulator, &mut self.input_info);

        self.emulator.take_snapshot().unwrap();
    }

    fn load_initial_state(&mut self) {
        // Create one component marker input such that the learner can create new tcp streams.
        let mut new_tcp_stream_testcase = TestCase::default();
        new_tcp_stream_testcase
            .component_markers
            .push(ComponentMarker::NewTcpStream);
        self.state.test_cases.insert(new_tcp_stream_testcase);

        // load all the inputs in the corpus
        let mut corpus_dir = self.config.context_path.clone();
        corpus_dir.push("corpus");
        for file in corpus_dir.read_dir().unwrap().flatten() {
            if file.file_type().unwrap().is_file() {
                let input_data_id = self
                    .state
                    .input_data
                    .insert(InputData::load_from_file(&file.path()));
                // TODO: Refactor this into more generic code and don't access private fields.
                let mut testcase = TestCase::default();
                testcase.tcp_streams.push(input_data_id);
                self.state.test_cases.insert(testcase);
            }
        }

        // We need to make sure that the learner has each input as prefix and suffix.
        for test_case_id in self.state.test_cases.keys() {
            self.state.learner.add_prefix([test_case_id]);
            self.state.learner.add_suffix([test_case_id]);
        }
    }

    fn build_state_model(
        &mut self,
        iteration: usize,
        rng: &mut impl Rng,
    ) {
        let threshold = if iteration == 0 {
            0.0
        } else {
            0.75 - 0.75 * 0.8f64.powi(iteration as i32)
        };
        let mut dot_path = self.config.context_path.clone();
        dot_path.push("states.dot");

        while self.state.learner.missing_observation_percentage() > threshold {
            info!(
                "Current observation percentage: {} (Threshold: {})",
                self.state.learner.missing_observation_percentage(),
                threshold
            );
            let mut current_row = self
                .state
                .learner
                .get_row_from_prefix(self.state.current_prefix.iter().copied())
                .unwrap();
            // there are still some missing observations somewhere.
            while let Some((row_id, path_from_current)) = self
                .state
                .learner
                .random_row_with_missing_observations(Some(current_row), rng)
            {
                for symbol in path_from_current {
                    self.advance_symbol(symbol);
                }

                self.perform_observations_for_current_state(row_id);

                current_row = row_id;
                if self.config.detailed_graph_building {
                    self.state.learner.make_consistent_and_closed();
                    self.export_learned_statemachine(&dot_path);
                    debug!(
                        "Table size: U{} | L{} x {}",
                        self.state.learner.upper_size(),
                        self.state.learner.lower_size(),
                        self.state.learner.table_width()
                    );
                }
            }
            // Go back one level
            if !self.state.current_prefix.is_empty() {
                self.emulator
                    .discard_current_and_restore_previous_snapshot()
                    .unwrap();
                self.state.current_prefix.pop();
            }

            if self.state.learner.missing_observation_percentage() <= threshold {
                self.state.learner.make_consistent_and_closed();
                self.export_learned_statemachine(&mut dot_path);
                info!("Symbols applied: {}", self.state.symbols_applied);
                info!(
                    "Symbols per second: {}",
                    self.state.symbols_applied as f64 / self.start_time.elapsed().as_secs_f64()
                );
                info!("Time elapsed: {:?}", self.start_time.elapsed());
                info!("Upper table size: {}", self.state.learner.upper_size());
                info!("Lower table size: {}", self.state.learner.lower_size());
                info!("Total distinct observations: {}", self.state.observations.len())
            }
        }
    }

    fn export_learned_statemachine(
        &mut self,
        dot_path: &Path,
    ) {
        self.state
            .learner
            .export_with_formatter(dot_path, &self.state.test_cases, |out, symbol| {
                write!(out, "[").unwrap();
                symbol.component_markers.iter().for_each(|marker| {
                    write!(out, "{marker:?}").unwrap();
                });
                symbol
                    .tcp_streams
                    .iter()
                    .map(|data_id| &self.state.input_data[*data_id].source)
                    .map(|source| match source {
                        InputDataSource::File(file) => file.file_name().unwrap().to_str().unwrap(),
                        InputDataSource::Unknown => "Unknown",
                    })
                    .for_each(|source| {
                        write!(out, "{source}, ").unwrap();
                    });
                write!(out, "]").unwrap();
            });
    }

    fn perform_observations_for_current_state(
        &mut self,
        row_id: ObservationRowId,
    ) {
        while let Some(suffix) = self.state.learner.next_unobserved_suffix(row_id) {
            let mut symbols = self
                .state
                .learner
                .get_suffix_symbols(suffix)
                .collect::<Vec<_>>()
                .into_iter()
                .peekable();
            while let Some(symbol) = symbols.next() {
                if symbols.peek().is_none() {
                    // gather observations
                    Self::clear_observers(&mut self.observers);
                    self.behavior_observer.borrow_mut().clear();
                    self.apply_symbol(symbol);
                    Self::gather_observations(&mut self.observers);
                    self.behavior_observer.borrow_mut().gather();
                    if Self::reduce_is_interesting(&mut self.is_interesting_reducers) {
                        debug!("Found interesting input !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                    }
                    // last
                } else {
                    // not last
                    self.apply_symbol(symbol);
                }
            }
            let behavior = self.suffix_behavior_reducer.borrow_mut().reduce().unwrap();

            let observation_id = self
                .state
                .observation_mapping
                .entry(behavior)
                .or_insert_with(|| {
                    self.state
                        .observations
                        .iter()
                        // Find all observations that are closer than a threshold
                        .filter_map(|(observation_id, observation)| {
                            let distance = observation.edge_hash_map.compare(&behavior.edge_hash_map) as u64;
                            (distance <= self.config.threshold).then_some((observation_id, distance))
                        })
                        // Get the minimally distant one
                        .min_by_key(|(_, distance)| *distance)
                        .map(|(observation_id, _)| observation_id)
                        // or create a new id if no observation in the threshold could be found
                        .unwrap_or_else(|| self.state.observations.insert(behavior))
                });
            self.state
                .learner
                .add_observation(row_id, suffix, *observation_id)
                .unwrap();
            // self.state.learner.add_observation(
            //     row_id,
            //     suffix,
            //     FuzzyObservation::new(behavior, self.config.threshold as u8),
            // );
            self.emulator.restore_snapshot().unwrap();
        }
    }

    /// Advances the current emulator state into the one represented by applying the provided symbol.
    fn advance_symbol(
        &mut self,
        symbol: SymbolId,
    ) {
        self.state.current_prefix.push(symbol);
        self.state
            .learner
            .add_prefix(self.state.current_prefix.iter().copied());

        self.apply_symbol(symbol);

        self.emulator.take_snapshot().unwrap();
    }

    fn apply_symbol(
        &mut self,
        symbol: SymbolId,
    ) {
        Self::update_input_info(&self.emulator, &mut self.input_info);

        let testcase = self.state.test_cases.get_mut(symbol).unwrap();
        let mut tcp = self.emulator.system().tcp().borrow_mut();
        // TODO: Make this configurable or adjustable by the fuzzer. For now only create streams if none are already available
        if self.input_info.tcp.streams_writable.is_empty() {
            for marker in &testcase.component_markers {
                if let ComponentMarker::NewTcpStream = marker {
                    // TODO: How to decide which one of the listeners to use? The fuzzer most likely needs to factor
                    //       in this knowledge into its fuzzing strategy, i.e. it needs to keep track of streams?
                    //       For now we just pick the first one we find.
                    let listener_id = tcp.sockets().find(|(_, sock)| sock.is_listener());
                    if listener_id.is_some() {
                        let listener_id = *listener_id.unwrap().0;
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
        }
        // Apply all the inputs, skipping over empty streams
        for (input_data_id, socket_id) in testcase
            .tcp_streams
            .iter()
            .copied()
            .zip(self.input_info.tcp.streams_writable.iter().copied())
        {
            let data = self.state.input_data.get(input_data_id).unwrap();
            let sock = tcp.socket_mut(socket_id).unwrap();
            trace!("Applying data from {:?}", data.source);
            let _ = sock.push_data(&data.data);
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
        assert!(matches!(status, ExitReason::Yielded));
        testcase.executions += 1;
        self.state.symbols_applied += 1;
    }

    fn fuzz(&mut self) {
        for (input_data_id, input_data) in self.state.input_data.iter() {
            println!("{:?}, {:?}", input_data_id, input_data.source);
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
            .unwrap_or(false)
    }

    fn clear_observers(observers: &mut [Rc<RefCell<dyn DebuggableObserver>>]) {
        for observer in observers.iter_mut() {
            observer.borrow_mut().clear();
        }
    }

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
}
