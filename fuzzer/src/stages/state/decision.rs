use std::marker::PhantomData;

use crate::executor::emulator::FUZEExecutor;
use libafl::corpus::CorpusId;
use libafl::prelude::{
    UsesInput,
    UsesState,
};
use libafl::stages::Stage;
use libafl_bolts::Error;

#[derive(Debug, Default)]
pub struct StateDecisionStage<StateT> {
    _phantom: PhantomData<StateT>,
}

impl<StateT> StateDecisionStage<StateT> {
    pub fn new() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<StateT> UsesState for StateDecisionStage<StateT>
where
    StateT: UsesInput,
{
    type State = StateT;
}

impl<ExecutorT, ManagerT, FuzzerT, StateT> Stage<ExecutorT, ManagerT, FuzzerT> for StateDecisionStage<StateT>
where
    StateT: UsesInput,
    FuzzerT: UsesState<State = StateT>,
    ManagerT: UsesState<State = StateT>,
    ExecutorT: UsesState<State = StateT> + FUZEExecutor,
{
    fn perform(
        &mut self,
        fuzzer: &mut FuzzerT,
        executor: &mut ExecutorT,
        state: &mut Self::State,
        manager: &mut ManagerT,
        corpus_idx: CorpusId,
    ) -> Result<(), Error> {
        todo!()
    }
}
