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

use fuzzyemu::emulator::abstractions::cpu::CPU;
use fuzzyemu::emulator::abstractions::emulator::{
    Scheduler,
    SchedulerError,
};
use fuzzyemu::emulator::abstractions::kernel_task::KernelTask;
use fuzzyemu::emulator::abstractions::mmu::MMU;
use fuzzyemu::emulator::abstractions::network::tcp_system::TCPSystem;
use fuzzyemu::emulator::abstractions::restore::Restorable;
use fuzzyemu::emulator::system::fd_map::FdId;
use fuzzyemu::emulator::system::System;
use fuzzyemu::ByteOrder;

use crate::inputs::multi_stream::MultiStreamInput;

#[derive(Debug)]
pub struct EmulatorExecutor<
    StateT,
    ObserversTupleT,
    SchedulerT,
    FD,
    FDData,
    TaskId,
    KernelTaskT,
    Endianness,
    CpuT,
    TcpT,
> where
    ObserversTupleT: ObserversTuple<StateT>,
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    /// The observers, observing each run
    observers: ObserversTupleT,
    scheduler: SchedulerT,
    system: System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>,
    phantom: PhantomData<StateT>,
}

impl<'a, StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
    EmulatorExecutor<StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    ObserversTupleT: ObserversTuple<StateT>,
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    SchedulerT: Scheduler,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    pub fn new(
        observers: ObserversTupleT,
        _state: &StateT,
        mut scheduler: SchedulerT,
        system: System<FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>,
    ) -> Self {
        // Run until executable yields to scheduler, meaning it is waiting for input.
        if !matches!(scheduler.run(None, None), Err(SchedulerError::NoRunnableTasks)) {
            panic!()
        }
        scheduler.snapshot().unwrap();
        Self {
            observers,
            scheduler,
            system,
            phantom: Default::default(),
        }
    }
}

impl<'a, StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT> UsesState
    for EmulatorExecutor<StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    ObserversTupleT: ObserversTuple<StateT>,
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    SchedulerT: Scheduler,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    type State = StateT;
}

impl<
        'a,
        EventManagerT,
        FuzzerT,
        StateT,
        ObserversTupleT,
        SchedulerT,
        FD,
        FDData,
        TaskId,
        KernelTaskT,
        Endianness,
        CpuT,
        TcpT,
    > Executor<EventManagerT, FuzzerT>
    for EmulatorExecutor<StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    EventManagerT: UsesState<State = Self::State>,
    FuzzerT: UsesState<State = Self::State>,
    StateT: UsesInput + HasExecutions + Debug,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
    SchedulerT: Scheduler + Debug,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
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
        let tcp = self.system.tcp.borrow_mut();
        self.scheduler.restore().unwrap();
        Ok(ExitKind::Ok)
    }
}

impl<'a, StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT> UsesObservers
    for EmulatorExecutor<StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
    SchedulerT: Scheduler,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    type Observers = ObserversTupleT;
}

impl<'a, StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT> HasObservers
    for EmulatorExecutor<StateT, ObserversTupleT, SchedulerT, FD, FDData, TaskId, KernelTaskT, Endianness, CpuT, TcpT>
where
    StateT: UsesInput,
    StateT::Input: MultiStreamInput,
    ObserversTupleT: ObserversTuple<StateT>,
    SchedulerT: Scheduler,
    FD: FdId,
    FDData: Default + Restorable,
    TaskId: Ord + Copy + Clone + Default,
    KernelTaskT: KernelTask<TaskId> + Debug,
    Endianness: ByteOrder,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn observers(&self) -> &Self::Observers {
        &self.observers
    }

    fn observers_mut(&mut self) -> &mut Self::Observers {
        &mut self.observers
    }
}
