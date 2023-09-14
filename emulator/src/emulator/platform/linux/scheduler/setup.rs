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

        let task_manager = self.system.task_manager.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut task_manager = task_manager.borrow_mut();
                let task = task_manager
                    .current_task_mut()
                    .expect("The current task must be in the set of tasks");
                syscall(cpu, task);
            }),
        );

        let vtlb_memory = self.system.memory.clone();
        let vtlb_task_manager = self.system.task_manager.clone();
        cpu.add_vtlb_hook(Box::new(move |uc, vaddr, _action| {
            let mut task_manager = vtlb_task_manager.borrow();
            let tlb_entry = task_manager.current_task().and_then(|task| {
                vtlb_memory
                    .borrow_mut()
                    .translate_vaddr(task.task_id(), vaddr)
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
        mut initial_task: LinuxKernelTask<Endianness, X86, CpuT>,
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
        let mut task_manager = self.system.task_manager.borrow_mut();

        let context_handle = cpu.create_context()?;
        let inserted = task_manager.new_task(initial_task, context_handle)?;
        cpu.save_context(inserted.context_handle())?;

        Ok(())
    }
}
