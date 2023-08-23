use super::*;

impl<'a, CpuT, Endianness> LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    pub(super) fn setup_cpu(&mut self) -> Result<(), SchedulerError> {
        let mut cpu = self.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_clock = self.system.clock.clone();
        cpu.add_block_hook(Box::new(move |_, _, instruction_count| {
            timestep_clock.borrow_mut().do_timestep(instruction_count);
        }));

        let syscall_task = self.context.borrow().current_task.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut task = syscall_task.borrow_mut();
                let task = task
                    .as_mut()
                    .expect("The current task must be set by the scheduler");
                task.yielded_syscall_rip.take();
                let result = syscall(cpu, &mut task.kernel);
                if let Err(rip) = result {
                    task.yielded_syscall_rip.insert(rip);
                }
            }),
        );

        let vtlb_memory = self.system.memory.clone();
        let vtlb_task = self.context.borrow().current_task.clone();
        cpu.add_vtlb_hook(Box::new(move |uc, vaddr, _action| {
            let task = vtlb_task.borrow();
            task.as_ref()
                .map(|task| task.kernel.task_id())
                .and_then(|task_id| {
                    vtlb_memory
                        .borrow_mut()
                        .translate_vaddr(task_id, vaddr)
                        .ok()
                })
        }))?;

        cpu.add_unmapped_mem_hook(
            1.into(),
            0.into(),
            Box::new(move |_, action, addr, size, _| {
                trace!("{:?} on {:#X} with size {} ", action, addr, size);
                false
            }),
        );

        Ok(())
    }

    pub(super) fn setup_initial_task(
        &mut self,
        mut initial_task: LinuxKernelTask<Endianness, X86>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), SchedulerError> {
        initial_task.add_stdio_handles();

        let mut cpu = self.cpu.borrow_mut();
        // TODO: Do more specific error mapping
        initial_task
            .load_binary(&mut *cpu, binary_path.as_path(), binary_args, binary_env)
            .map_err(|_| SchedulerError::Unknown)?;
        debug!("Loaded initial binary");
        let mut scheduler_context = self.context.borrow_mut();
        let task = Task {
            kernel: initial_task,
            state: TaskState::Runnable,
            cpu_context_handle: cpu.create_context()?,
            yielded_syscall_rip: None,
        };
        cpu.save_context(task.cpu_context_handle)?;
        scheduler_context
            .tasks
            .try_insert(task.kernel.task_id(), task)
            .map_err(|_| CPUError::TaskExists)?;
        Ok(())
    }
}
