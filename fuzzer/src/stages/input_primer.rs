use std::marker::PhantomData;

use libafl::corpus::{
    Corpus,
    CorpusId,
};
use libafl::inputs::UsesInput;
use libafl::prelude::UsesState;
use libafl::stages::Stage;
use libafl::state::HasCorpus;
use libafl_bolts::Error;

use fuzzyemu::emulator::abstractions::file_descriptors::FileOperationsReady;
use fuzzyemu::emulator::abstractions::system::SystemControl;
use fuzzyemu::emulator::interface::network::tcp::{
    TCPSocket,
    TCPSystemControl,
};

use crate::executor::emulator::FUZEExecutor;
use crate::inputs::emulator::EmulatorInputComponent;
use crate::inputs::multi_stream::MultiStreamInput;

/// Primes the currently selected input such that it contains all components
/// that the executable is currently awaiting operations on. The fuzzer can
/// then decide whether or not it wants to fulfil these requests. However,
/// at least one of the requests has to be met, otherwise the emulation will
/// return without doing anything, since no processes will be able to awake.
#[derive(Debug, Default)]
pub struct InputPrimerStage<StateT> {
    _phantom: PhantomData<StateT>,
}

impl<StateT> InputPrimerStage<StateT> {
    pub fn new() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<StateT> UsesState for InputPrimerStage<StateT>
where
    StateT: UsesInput,
{
    type State = StateT;
}

impl<ExecutorT, ManagerT, FuzzerT, StateT> Stage<ExecutorT, ManagerT, FuzzerT> for InputPrimerStage<StateT>
where
    StateT: HasCorpus,
    <<StateT as HasCorpus>::Corpus as UsesInput>::Input: MultiStreamInput<ComponentId = EmulatorInputComponent>,
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
        let mut testcase = state.corpus().get(corpus_idx)?.borrow_mut();
        let input = testcase.input_mut().as_mut().ok_or(Error::unknown(""))?;
        let interest_count = tcp.sockets().fold(0usize, |acc, (id, socket)| {
            let interest = socket.interest();
            if interest.intersects(
                FileOperationsReady::ReadOutOfBand
                    | FileOperationsReady::Read
                    | FileOperationsReady::ReadNormal
                    | FileOperationsReady::ReadOutOfBand,
            ) {
                acc + 1
            } else {
                acc
            }
        });
        input.set_available_streams(&EmulatorInputComponent::Tcp, interest_count);
        Ok(())
    }
}
