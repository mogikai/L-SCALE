//! This module contains all the emulator components of fuzzyemu.
//! TODO: Detailed documentation
use std::time::Duration;

use crate::emulator::abstractions::emulator::Scheduler;
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::abstractions::system::SystemControl;
use crate::emulator::interface::emulator::{
    Emulator,
    ExitReason,
};
use crate::emulator::interface::error::{
    EmulatorError,
    SchedulerError,
    StopReason,
};
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::trace::trace_hook;

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
        Self { scheduler, system }
    }
}

impl<SchedulerT, SystemT> Restorable for GenericEmulator<SchedulerT, SystemT>
where
    SchedulerT: Restorable,
    SystemT: Restorable,
{
    fn snapshot(&mut self) -> Result<(), GenericError> {
        self.scheduler.snapshot()?;
        self.system.snapshot()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        self.scheduler.restore_and_discard()?;
        self.system.restore_and_discard()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        self.scheduler.restore()?;
        self.system.restore()
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
        match self.scheduler.run(timeout, instruction_count) {
            Ok(_) => Ok(ExitReason::Stopped),
            Err(e) => match e {
                SchedulerError::Unknown => Err(EmulatorError::Unknown),
                SchedulerError::Stopped(reason) => match reason {
                    StopReason::Unknown => Ok(ExitReason::Stopped),
                    StopReason::Killed(_) => todo!(),
                    StopReason::MissingSyscall(_) => todo!(),
                    StopReason::ProcessExit(_) => todo!(),
                    StopReason::Yield(_) => Ok(ExitReason::Yielded),
                    StopReason::Breakpoint => Ok(ExitReason::Breakpoint),
                },
                SchedulerError::CPUError(e) => Err(EmulatorError::from(e)),
                SchedulerError::NoRunnableTasks => Ok(ExitReason::Yielded),
            },
        }
    }

    fn system(&self) -> &impl SystemControl {
        &self.system
    }

    fn enable_trace(&mut self) -> Result<(), EmulatorError> {
        let trace_memory = self.system.memory().clone();
        let task_manager = self.system.task_manager().clone();
        self.system.cpu().borrow_mut().add_code_hook(
            1.into(),
            0.into(),
            Box::new(move |uc, address, size| {
                let mut memory = trace_memory.borrow_mut();
                trace_hook(uc, &mut memory, task_manager.borrow().current_task().unwrap().task_id(), address, size)
            }),
        )?;
        Ok(())
    }
}
