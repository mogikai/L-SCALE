use std::fmt::Debug;
use std::marker::PhantomData;

use libafl::executors::{
    Executor,
    ExitKind,
    HasObservers,
};
use libafl::inputs::UsesInput;
use libafl::observers::{
    ObserversTuple,
    UsesObservers,
};
use libafl::prelude::UsesState;
use libafl::state::HasExecutions;
use libafl_bolts::Error;

use fuzzyemu::emulator::abstractions::emulator::{
    Scheduler,
    SchedulerError,
};
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::emulator::abstractions::system::SystemControl;

use crate::inputs::multi_stream::MultiStreamInput;

pub trait FUZEExecutor {
    fn scheduler(&self) -> &impl Scheduler;
    fn scheduler_mut(&mut self) -> &mut impl Scheduler;
    fn system(&self) -> &impl SystemControl;
    fn system_mut(&mut self) -> &mut impl SystemControl;
}

#[derive(Debug)]
pub struct DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT> {
    /// The observers, observing each run
    observers: ObserversTupleT,
    scheduler: SchedulerT,
    system: SystemT,
    phantom: PhantomData<StateT>,
}

impl<StateT, ObserversTupleT, SchedulerT, SystemT> DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    ObserversTupleT: ObserversTuple<StateT>,
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    SchedulerT: Scheduler,
    SystemT: SystemControl,
{
    pub fn new(
        observers: ObserversTupleT,
        _state: &StateT,
        mut scheduler: SchedulerT,
        system: SystemT,
    ) -> Self {
        // Run until executable yields to scheduler, meaning it is waiting for input.
        if !matches!(scheduler.run(None, None), Err(SchedulerError::NoRunnableTasks)) {
            panic!()
        }
        // TODO: Move this to state manager stage
        scheduler.snapshot().unwrap();
        Self {
            observers,
            scheduler,
            system,
            phantom: Default::default(),
        }
    }
}

impl<StateT, ObserversTupleT, SchedulerT, SystemT> FUZEExecutor
    for DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    SchedulerT: Scheduler,
    SystemT: SystemControl,
{
    fn scheduler(&self) -> &impl Scheduler {
        &self.scheduler
    }

    fn scheduler_mut(&mut self) -> &mut impl Scheduler {
        &mut self.scheduler
    }

    fn system(&self) -> &impl SystemControl {
        &self.system
    }

    fn system_mut(&mut self) -> &mut impl SystemControl {
        &mut self.system
    }
}

impl<StateT, ObserversTupleT, SchedulerT, SystemT> UsesState
    for DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    ObserversTupleT: ObserversTuple<StateT>,
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
{
    type State = StateT;
}

impl<EventManagerT, FuzzerT, StateT, ObserversTupleT, SchedulerT, SystemT> Executor<EventManagerT, FuzzerT>
    for DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    EventManagerT: UsesState<State = Self::State>,
    FuzzerT: UsesState<State = Self::State>,
    StateT: UsesInput + HasExecutions,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
    SchedulerT: Scheduler,
    SystemT: SystemControl,
{
    fn run_target(
        &mut self,
        fuzzer: &mut FuzzerT,
        state: &mut Self::State,
        mgr: &mut EventManagerT,
        input: &Self::Input,
    ) -> Result<ExitKind, Error> {
        *state.executions_mut() += 1;
        let result = self.scheduler.run(None, None);
        Ok(ExitKind::Ok)
    }
    fn post_run_reset(&mut self) {
        self.scheduler.restore().unwrap();
    }
}

impl<StateT, ObserversTupleT, SchedulerT, SystemT> UsesObservers
    for DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
{
    type Observers = ObserversTupleT;
}

impl<StateT, ObserversTupleT, SchedulerT, SystemT> HasObservers
    for DefaultFUZEExecutor<StateT, ObserversTupleT, SchedulerT, SystemT>
where
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
