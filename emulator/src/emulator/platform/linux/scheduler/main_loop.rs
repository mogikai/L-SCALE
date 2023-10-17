use super::*;
use crate::emulator::system::task_manager::TaskState;

impl<'a, CpuT, TcpT, Endianness> LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    TcpT: TCPSystem,
    Endianness: ByteOrder + 'static,
{
    pub(super) fn scheduler_loop(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError> {
        loop {
            self.schedule()?;
            self.pre_task()?;
            let stop_reason = self.process_task(timeout, instruction_count)?;
            self.post_task(stop_reason)?;
        }
    }

    fn schedule(&mut self) -> Result<(), SchedulerError> {
        // TODO: Implement actual scheduling. Potentially delegate to fuzzer.
        let mut task_manager = self.system.task_manager.borrow_mut();
        let task_id = task_manager
            .tasks()
            .find(|task| matches!(task.state(), TaskState::Runnable | TaskState::WakeUp(_)))
            .ok_or(SchedulerError::NoRunnableTasks)?
            .task_id();

        task_manager.set_current_task(task_id);
        Ok(())
    }

    fn pre_task(&mut self) -> Result<(), SchedulerError> {
        let mut task_manager = self.system.task_manager.borrow_mut();
        let mut cpu = self.system.cpu.borrow_mut();

        let task = task_manager
            .current_task()
            .expect("Scheduled task must exist");

        cpu.restore_context(task.context_handle())
            .map_err(|_| StopReason::Unknown)?;

        if let TaskState::WakeUp(rip) = task.state() {
            cpu.reg_write(RegisterX86::RIP.into(), rip.into())?;
        }

        Ok(())
    }

    fn process_task(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<Option<StopReason>, CPUError> {
        // We make sure that cpu is not already mutably borrowed.
        // Then we take the pointer and call emulate, such that callbacks inside
        // the emulation can still borrow the cpu where needed.
        let cpu = unsafe {
            self.system.cpu.borrow_mut();
            &mut *self.system.cpu.as_ptr()
        };
        let result = cpu.emulate(None, timeout, instruction_count);
        match result {
            Err(CPUError::Stopped(reason)) => Ok(Some(reason)),
            Err(e) => Err(e),
            _ => Ok(None),
        }
    }

    fn post_task(
        &mut self,
        stop_reason: Option<StopReason>,
    ) -> Result<(), SchedulerError> {
        // Save all the context after we are done executing the scheduled task
        let mut task_manager = self.system.task_manager.borrow_mut();
        let mut cpu = self.system.cpu.borrow_mut();

        let task = task_manager
            .current_task_mut()
            .expect("The current task must exist since it was scheduled");
        cpu.save_context(task.context_handle())
            .map_err(|_| StopReason::Unknown)?;

        let error = match stop_reason {
            None => Ok(()),
            Some(StopReason::Yield(rip)) => Ok(()),
            Some(StopReason::Killed(signal)) => {
                // TODO: Remove Task from list of runnable tasks
                Err(StopReason::Killed(signal))
            }
            Some(reason) => Err(reason),
        };

        Ok(error?)
    }
}
