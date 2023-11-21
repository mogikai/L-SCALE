use std::marker::PhantomData;

use crate::executor::emulator::FUZEExecutor;
use libafl::corpus::CorpusId;
use libafl::prelude::UsesState;
use libafl::stages::Stage;
use libafl_bolts::Error;

use crate::state::FUZEState;

/// Primes the currently selected input such that it contains all components
/// that the executable is currently awaiting operations on. The fuzzer can
/// then decide whether or not it wants to fulfil these requests. However,
/// at least one of the requests has to be met, otherwise the emulation will
/// return without doing anything, since no processes will be able to awake.
#[derive(Debug, Default)]
pub struct InputPrimerStage<StateT> {
    _phantom: PhantomData<StateT>,
}

impl<StateT> UsesState for InputPrimerStage<StateT> {
    type State = StateT;
}

impl<ExecutorT, ManagerT, FuzzerT, StateT> Stage<ExecutorT, ManagerT, FuzzerT> for InputPrimerStage<StateT>
where
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
        let tcp = executor.system().tcp().borrow_mut();

        todo!()
    }
}
