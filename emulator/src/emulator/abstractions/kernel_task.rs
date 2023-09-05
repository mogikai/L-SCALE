use crate::emulator::abstractions::cpu::address::EVAddr;

pub trait KernelTask<TaskId> {
    fn task_id(&self) -> TaskId;
    fn entrypoint(&self) -> Option<EVAddr>;
    fn load_base(&self) -> Option<EVAddr>;
}
