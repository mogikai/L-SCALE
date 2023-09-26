use super::*;

impl<'a, CpuT, TcpT, Endianness> LinuxX86Scheduler<'a, CpuT, TcpT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    TcpT: TCPSystem + 'a,
    Endianness: ByteOrder + 'static,
{
    pub(super) fn setup_cpu(&mut self) -> Result<(), SchedulerError> {
        let mut cpu = self.system.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_clock = self.system.clock.clone();
        cpu.add_block_hook(Box::new(move |_, _, instruction_count| {
            timestep_clock.borrow_mut().do_timestep(instruction_count);
        }));

        let syscall_system = self.system.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move || {
                // TODO: Currently the task manager cannot be borrowed anywhere else because
                //       it is borrowed here. Need to find some way to allow the task itself
                //       to for example add new tasks spawned with fork()
                let mut task_manager = syscall_system.task_manager.borrow_mut();
                let task = task_manager
                    .current_task_mut()
                    .expect("The current task must be in the set of tasks");
                syscall(&syscall_system, task);
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
        mut initial_task: LinuxKernelTask<Endianness, X86, CpuT, TcpT>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), SchedulerError> {
        initial_task.add_stdio_handles();

        // TODO: Do more specific error mapping
        initial_task
            .load_binary(binary_path.as_path(), binary_args, binary_env)
            .map_err(|_| SchedulerError::Unknown)?;
        debug!("Loaded initial binary");

        let mut task_manager = self.system.task_manager.borrow_mut();
        let mut cpu = self.system.cpu.borrow_mut();
        let context_handle = cpu.create_context()?;
        let inserted = task_manager.new_task(initial_task, context_handle)?;
        cpu.save_context(inserted.context_handle())?;

        Ok(())
    }
}
