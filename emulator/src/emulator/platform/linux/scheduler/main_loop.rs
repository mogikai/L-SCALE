use super::*;

impl<'a, CpuT, Endianness> LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    pub(super) fn scheduler_loop(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), SchedulerError> {
        loop {
            let task_index = self.schedule()?;

            self.pre_task(task_index)?;
            let stop_reason = self.process_task(timeout, instruction_count)?;
            self.post_task(stop_reason)?;
        }
    }

    fn schedule(&mut self) -> Result<usize, SchedulerError> {
        // TODO: Implement actual scheduling. Potentially delegate to fuzzer.
        self.context
            .borrow_mut()
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(i, task_entry)| task_entry.task().map(|t| (i, t)))
            .find(|(i, task)| matches!(task.state, TaskState::Runnable))
            .map(|(i, _)| i)
            .ok_or(SchedulerError::NoRunnableTasks)
    }

    fn pre_task(
        &mut self,
        task_index: usize,
    ) -> Result<(), SchedulerError> {
        let mut context = self.context.borrow_mut();
        let mut cpu = self.cpu.borrow_mut();

        let task = context
            .tasks
            .get(task_index)
            .expect("Scheduled task must exist")
            .task()
            .expect("The task cannot be deleted since it was scheduled");

        cpu.restore_context(task.cpu_context_handle)
            .map_err(|_| StopReason::Unknown)?;

        if let Some(rip) = task.yielded_syscall_rip {
            cpu.reg_write(RegisterX86::RIP.into(), rip).unwrap();
        }
        context.current_task.borrow_mut().insert(task_index);
        Ok(())
    }

    fn process_task(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<Option<StopReason>, CPUError> {
        let result = self
            .cpu
            .borrow_mut()
            .emulate(None, timeout, instruction_count);
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
        let mut context = self.context.borrow_mut();
        let mut cpu = self.cpu.borrow_mut();

        let task = context
            .current_task_mut()
            .expect("The current task must exist since it was scheduled");
        cpu.save_context(task.cpu_context_handle)
            .map_err(|_| StopReason::Unknown)?;

        let error = match stop_reason {
            None => Ok(()),
            Some(StopReason::Yield) => {
                task.state = TaskState::Yielded;
                Ok(())
            }
            Some(StopReason::Killed(signal)) => {
                // TODO: Remove Task from list of runnable tasks
                Err(StopReason::Killed(signal))
            }
            Some(reason) => Err(reason),
        };

        // Remove current task so that a new one can be scheduled
        let task_id = context.current_task.take().unwrap();

        Ok(error?)
    }
}
