use mogi_system::components::mmu::{
    MMU,
    MemoryAction,
};
use mogi_system::errors::cpu::CPUError;
use mogi_utilities::addresses::EVAddr;
use tracing::{
    debug,
    trace,
};

use super::*;

impl<'a, CpuT, MmuT, TcpT, Endianness> LinuxX86Scheduler<'a, CpuT, MmuT, TcpT, Endianness>
where
    CpuT: CPU + Hooks + Restorable + 'static,
    MmuT: MMU + 'static,
    TcpT: TCPSystem + 'static,
    Endianness: ByteOrder + 'static,
{
    pub(super) fn setup_cpu(&mut self) -> Result<(), SchedulerError> {
        let mut cpu = self.system.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_clock = self.system.clock.clone();
        cpu.add_block_hook(
            EVAddr::from(1u64),
            EVAddr::from(0u64),
            Box::new(move |_, _, instruction_count| {
                timestep_clock.borrow_mut().do_timestep(instruction_count);
            }),
        );

        let syscall_system = self.system.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1u64),
            EVAddr::from(0u64),
            Box::new(move || {
                let mut task_manager = syscall_system.task_manager.borrow_mut();
                let task = task_manager
                    .current_task_mut()
                    .expect("The current task must be in the set of tasks");
                syscall(&syscall_system, task);
            }),
        );

        drop(cpu);
        let mut mmu = self.system.mmu.borrow_mut();
        let vtlb_memory = self.system.memory.clone();
        let vtlb_task_manager = self.system.task_manager.clone();
        mmu.set_page_fault_callback(Box::new(move |addr, action| {
            // page fault either due to 1. unmapped memory or 2. invalid permissions
            let task_manager = vtlb_task_manager.borrow();
            let current_task = task_manager
                .current_task()
                .expect("If there was a page fault there has to be a task");
            vtlb_memory
                .borrow_mut()
                .translate_vaddr(current_task.task_id(), addr)
                .ok()
                // TODO COW (find out if mapping did exist and match permissions)
        }))
        .map_err(|_| SchedulerError::CPUError(CPUError::Unknown))?;

        Ok(())
    }

    pub(super) fn setup_initial_task(
        &mut self,
        mut initial_task: LinuxKernelTask<Endianness, X86, CpuT, MmuT, TcpT>,
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
