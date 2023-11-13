use std::marker::PhantomData;
use std::time::Duration;

use libafl::corpus::Corpus;
use libafl::feedbacks::Feedback;
use libafl::inputs::{
    Input,
    UsesInput,
};
use libafl_bolts::rands::Rand;
use libafl_bolts::serdeany::{
    NamedSerdeAnyMap,
    SerdeAnyMap,
};
use libafl_bolts::Error;
use serde::de::DeserializeOwned;
use serde::{
    Deserialize,
    Serialize,
};

pub mod libafl_traits;

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound = "
    InputT: Serialize + DeserializeOwned,
    CorpusT: Serialize + DeserializeOwned,
    RandomT: Serialize + DeserializeOwned,
    SolutionsT: Serialize + DeserializeOwned,
")]
pub struct FUZEState<InputT, CorpusT, RandomT, SolutionsT> {
    rand: RandomT,
    /// How many times the executor ran the harness/target
    executions: usize,
    /// At what time the fuzzing started
    start_time: Duration,
    /// the number of new paths that imported from other fuzzers
    imported: usize,
    /// The corpus
    corpus: CorpusT,
    // Solutions corpus
    solutions: SolutionsT,
    /// Metadata stored for this state by one of the components
    metadata: SerdeAnyMap,
    /// Metadata stored with names
    named_metadata: NamedSerdeAnyMap,
    /// MaxSize testcase size for mutators that appreciate it
    max_size: usize,
    // Performance statistics for this fuzzer
    // introspection_monitor: ClientPerfMonitor,
    // #[cfg(feature = "std")]
    // /// Remaining initial inputs to load, if any
    // remaining_initial_files: Option<Vec<PathBuf>>,
    // #[cfg(feature = "std")]
    // /// Remaining initial inputs to load, if any
    // dont_reenter: Option<Vec<PathBuf>>,
    /// The last time we reported progress (if available/used).
    /// This information is used by fuzzer `maybe_report_progress`.
    last_report_time: Option<Duration>,
    phantom: PhantomData<InputT>,
}

impl<InputT, CorpusT, RandomT, SolutionsT> FUZEState<InputT, CorpusT, RandomT, SolutionsT>
where
    InputT: Input,
    RandomT: Rand,
    CorpusT: Corpus<Input = <Self as UsesInput>::Input>,
    SolutionsT: Corpus<Input = <Self as UsesInput>::Input>,
{
    pub fn new<F, O>(
        rand: RandomT,
        corpus: CorpusT,
        solutions: SolutionsT,
        feedback: &mut F,
        objective: &mut O,
    ) -> Result<Self, Error>
    where
        F: Feedback<Self>,
        O: Feedback<Self>,
    {
        let mut state = Self {
            rand,
            executions: 0,
            start_time: Default::default(),
            imported: 0,
            corpus,
            solutions,
            metadata: Default::default(),
            named_metadata: Default::default(),
            max_size: 0,
            last_report_time: None,
            phantom: Default::default(),
        };
        feedback.init_state(&mut state)?;
        Ok(state)
    }
}
