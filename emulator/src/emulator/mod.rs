//! This module contains all the emulator components of fuzzyemu.
//! TODO: Detailed documentation
use mogi_system::components::cpu::hooks::Hooks;
use mogi_system::components::restore::Restorable;
use mogi_system::errors::cpu::StopReason;
use mogi_system::errors::{
    EmulatorError,
    SchedulerError,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{
    Duration,
    Instant,
};
use tracing::{
    Level,
    debug,
    event,
    span,
    trace,
};

use crate::emulator::abstractions::emulator::Scheduler;
use crate::emulator::abstractions::system::SystemControl;
use crate::emulator::interface::emulator::{
    Emulator,
    ExitReason,
};
use crate::emulator::interface::meter::{
    DebuggableMeter,
    Meter,
};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use mogi_system::errors::generic::GenericError;

pub mod abstractions;
pub mod interface;
pub mod macros;
pub mod platform;
pub mod system;
mod trace;

#[derive(Debug)]
pub struct GenericEmulator<SchedulerT, SystemT> {
    scheduler: SchedulerT,
    system: SystemT,
    meters: Vec<Rc<RefCell<dyn DebuggableMeter>>>,
}

impl<SchedulerT, SystemT> GenericEmulator<SchedulerT, SystemT>
where
    SchedulerT: Scheduler,
    SystemT: SystemControl,
{
    pub fn new(
        scheduler: SchedulerT,
        system: SystemT,
    ) -> Self {
        Self {
            scheduler,
            system,
            meters: vec![],
        }
    }
}

impl<SchedulerT, SystemT> Restorable for GenericEmulator<SchedulerT, SystemT>
where
    SchedulerT: Restorable,
    SystemT: Restorable,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        trace!("Taking emulator snapshot");
        self.scheduler.take_snapshot()?;
        self.system.take_snapshot()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        trace!("Discarding current and restoring previous emulator snapshot");
        self.scheduler
            .discard_current_and_restore_previous_snapshot()?;
        self.system.discard_current_and_restore_previous_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        trace!("Restoring emulator snapshot");
        self.scheduler.restore_snapshot()?;
        self.system.restore_snapshot()
    }
}

impl<SchedulerT, SystemT> Emulator for GenericEmulator<SchedulerT, SystemT>
where
    SchedulerT: Restorable + Scheduler,
    SystemT: Restorable + SystemControl<TaskIdT = TaskId>, // TODO: Remove the TaskId bound and make it generic
{
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<ExitReason, EmulatorError> {
        self.meters.iter().for_each(|m| m.borrow_mut().start());
        let start = Instant::now();
        let result = match self.scheduler.run(timeout, instruction_count) {
            Ok(_) => Ok(ExitReason::Stopped),
            Err(e) => match e {
                SchedulerError::Unknown => Err(EmulatorError::Unknown),
                SchedulerError::Stopped(reason) => match reason {
                    StopReason::Unknown => Ok(ExitReason::Stopped),
                    StopReason::Killed(_) => todo!(),
                    StopReason::MissingSyscall(_) => todo!(),
                    StopReason::ProcessExit(_) => Ok(ExitReason::Stopped),
                    StopReason::Yield(_) => Ok(ExitReason::Yielded),
                    StopReason::Breakpoint => Ok(ExitReason::Breakpoint),
                },
                SchedulerError::CPUError(e) => Err(EmulatorError::from(e)),
                SchedulerError::NoRunnableTasks => Ok(ExitReason::Yielded),
            },
        };
        let elapsed = start.elapsed();
        let _span = span!(Level::TRACE, "elapsed");
        let _span = _span.enter();
        event!(Level::TRACE, "Emulation took {} milliseconds", elapsed.as_millis());
        self.meters.iter().rev().for_each(|m| m.borrow_mut().stop());
        result
    }

    fn system(&self) -> &impl SystemControl {
        &self.system
    }

    fn enable_trace(&mut self) -> Result<(), EmulatorError> {
        let trace_memory = self.system.memory().clone();
        let task_manager = self.system.task_manager().clone();
        todo!();
        // self.system.cpu().borrow_mut().add_code_hook(
        //     1u64.into(),
        //     0u64.into(),
        //     Box::new(move |uc, address, size| {
        //         let mut memory = trace_memory.borrow_mut();
        //         trace_hook(uc, &mut memory, task_manager.borrow().current_task().unwrap().task_id(), address, size)
        //     }),
        // )?;
        Ok(())
    }

    fn register_meter(
        &mut self,
        meter: Rc<RefCell<dyn DebuggableMeter>>,
    ) {
        self.meters.push(meter);
    }
}
