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

        let syscall_sched_context = self.context.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut context = syscall_sched_context.borrow_mut();
                let task = context
                    .current_task_mut()
                    .expect("The current task must be in the set of tasks");
                task.yielded_syscall_rip.take();
                let result = syscall(cpu, &mut task.kernel);
                if let Err(rip) = result {
                    task.yielded_syscall_rip.insert(rip);
                }
            }),
        );

        let vtlb_memory = self.system.memory.clone();
        let vtlb_sched_context = self.context.clone();
        cpu.add_vtlb_hook(Box::new(move |uc, vaddr, _action| {
            let mut context = vtlb_sched_context.borrow_mut();
            let tlb_entry = context.current_task().and_then(|task| {
                vtlb_memory
                    .borrow_mut()
                    .translate_vaddr(task.kernel.task_id(), vaddr)
                    .ok()
            });
            tlb_entry
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

        // Insert the mapping. If this doesn't fail we can then insert the actual task at the end
        // of the tasks vector
        let task_index = scheduler_context.tasks.len();
        scheduler_context
            .task_map
            .try_insert(task.kernel.task_id(), task_index)
            .map_err(|_| CPUError::TaskExists)?;
        scheduler_context.tasks.push(task.into());
        Ok(())
    }
}
