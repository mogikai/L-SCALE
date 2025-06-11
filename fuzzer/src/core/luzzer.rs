use mogi_statelearner::{
    ObservationValue,
    StateLearner,
    WordId,
};
use slotmap::{
    new_key_type,
    SlotMap,
};

#[derive(Debug, PartialEq, Clone, Copy)]
struct Observation;

impl ObservationValue for Observation {}

new_key_type! { pub struct InputDataId; }

#[derive(Debug)]
enum ComponentMarker {
    NewTcpStream,
}

#[derive(Debug)]
pub struct TestCase {
    /// The number of times this test case was applied to the target
    executions: usize,
    component_markers: Vec<ComponentMarker>,
    tcp_streams: Vec<Option<InputDataId>>,
}

#[derive(Debug)]
pub struct Luzzer {
    inputs: SlotMap<WordId, TestCase>,
    input_data: SlotMap<InputDataId, Vec<u8>>,
    learner: StateLearner<Observation>,
}
