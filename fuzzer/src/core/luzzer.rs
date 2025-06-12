use std::cell::RefCell;
use std::fmt::{
    Debug,
    Display,
    Formatter,
};
use std::fs;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};

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
use mogi_statelearner::{
    ObservationValue,
    StateLearner,
    SymbolId,
};
use mogi_utilities::error::memory::MemoryError;
use serde::{
    Deserialize,
    Serialize,
};
use slotmap::{
    SlotMap,
    new_key_type,
};
use tracing::{
    debug,
    trace,
};

use crate::api::observer::DebuggableObserver;
use crate::api::reducer::Reducer;
use crate::core::fuzzer::SuffixBehavior;
use crate::observers::bb_based::bb::BlockHitObserver;
use crate::observers::bb_based::edge::EdgeHitCountObserver;
use crate::observers::emulator::basic_block::BasicBlockCollector;
use crate::reducers::bb::behavior::BasicBlockBehaviorReducer;
use crate::reducers::edge::InterestingEdgesReducer;

#[derive(Debug, PartialEq, Clone, Copy)]
struct Observation;

impl ObservationValue for Observation {}

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

#[derive(Debug)]
enum ComponentMarker {
    NewTcpStream,
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
        write!(f, "TestCase(exe: {}, tcp: {:?})", self.executions, self.tcp_streams)
    }
}

#[derive(Debug, Default)]
pub struct LuzzerState {
    test_cases: SlotMap<SymbolId, TestCase>,
    input_data: SlotMap<InputDataId, InputData>,
    learner: StateLearner<Observation>,
    /// The current prefix the learner has taken through the target.
    /// There exists a snapshot of the emulator for each prefix symbol.
    /// If the prefix is popped, the state of the emulator has to be reset.
    current_prefix: Vec<SymbolId>,
}

#[derive(Debug)]
pub struct Luzzer<EmulatorT> {
    state: LuzzerState,
    observers: Vec<Rc<RefCell<dyn DebuggableObserver>>>,
    block_observer: Rc<RefCell<BlockHitObserver>>,
    is_interesting_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    suffix_behavior_observer: Rc<RefCell<dyn DebuggableObserver>>,
    suffix_behavior_reducer: Rc<RefCell<dyn DebuggableReducer<Output = SuffixBehavior>>>,
    is_crash_reducers: Vec<Rc<RefCell<dyn DebuggableReducer<Output = bool>>>>,
    emulator: EmulatorT,
    context_path: PathBuf,
    input_info: EmulatorInputInfo,
}

impl<EmulatorT> Luzzer<EmulatorT>
where
    EmulatorT: Emulator,
{
    pub fn new(
        mut emulator: EmulatorT,
        context_path: PathBuf,
    ) -> Self {
        let memory = emulator.system().memory().borrow();
        let exe_mappings = memory
            .mappings(&TaskId { pid: 1337, tid: 1 })
            .filter_map(|mapping| {
                mapping
                    .label
                    .as_ref()
                    .and_then(|label| label.contains("open62541").then_some(mapping))
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
        let edge_observer = Rc::new(RefCell::new(EdgeHitCountObserver::new()));
        let edge_reducer = Rc::new(RefCell::new(InterestingEdgesReducer::new(edge_observer.clone())));
        let behavior_observer = BlockHitObserver::new();
        let bb_collector = Rc::new(RefCell::new(BasicBlockCollector::new(
            emulator.system().cpu(),
            start,
            end,
            [
                &behavior_observer.shared_observe(),
                &edge_observer.borrow().shared_observe(),
            ]
            .into_iter(),
        )));
        emulator.register_meter(bb_collector);
        let behavior_observer = Rc::new(RefCell::new(behavior_observer));
        let behavior_reducer = Rc::new(RefCell::new(BasicBlockBehaviorReducer::new(behavior_observer.clone())));

        Self {
            state: Default::default(),
            observers: vec![edge_observer],
            block_observer: behavior_observer.clone(),
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
        self.build_state_model();
        while do_fuzzing.load(Ordering::SeqCst) {
            self.fuzz();
        }
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

        self.emulator.take_snapshot().unwrap();
    }

    fn load_initial_state(&mut self) {
        // self.create_component_marker_inputs();
        // FIXME: For now, we simply create one tcp stream in the beginning. The fuzzer is not allowed to create
        //        any new streams at the moment (during fuzzing)
        let mut tcp = self.emulator.system().tcp().borrow_mut();
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

        let mut corpus_dir = self.context_path.clone();
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

        for test_case_id in self.state.test_cases.keys() {
            self.state.learner.add_prefix([test_case_id]);
            self.state.learner.add_suffix([test_case_id]);
        }
    }

    fn build_state_model(&mut self) {
        let mut dot_path = self.context_path.clone();
        dot_path.push("states.dot");

        let current_row = self
            .state
            .learner
            .get_row_from_prefix(self.state.current_prefix.iter().copied())
            .unwrap();

        while let Some((row_id, path_from_current)) = self
            .state
            .learner
            .next_row_with_missing_observations(Some(current_row))
        {
            for symbol in path_from_current {
                self.advance_symbol(symbol);
            }

            // TODO: Do the discovery steps

            self.state.learner.unobserved_suffixes(row_id);

            self.state.learner.make_closed();
            self.state.learner.make_consistent();
            trace!("{}", self.state.learner);
            self.state.learner.export(&dot_path, &self.state.test_cases);
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
        self.emulator.take_snapshot().unwrap();
    }

    fn fuzz(&mut self) {
        todo!()
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
