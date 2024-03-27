mod restorable;

use crate::emulator::abstractions::kernel_task::KernelTask;
use mogi_system::components::cpu::hooks::Hooks;

use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::task_manager::restorable::{
    TaskManagerSnapshot,
    TaskSnapshot,
};
use mogi_system::components::cpu::ContextHandle;
use mogi_system::errors::cpu::CPUError;
use mogi_utilities::addresses::EVAddr;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Try;
use std::rc::Rc;

#[derive(Debug, Copy, Clone)]
pub enum TaskState {
    Runnable,
    WakeUp(EVAddr),
    Yielded(EVAddr),
}

#[derive(Debug)]
pub struct Task<TaskId, T: KernelTask<TaskId>> {
    kernel: T,
    state: TaskState,
    cpu_context_handle: ContextHandle,
    snapshots: Vec<TaskSnapshot>,
    _phantom: PhantomData<TaskId>,
}

impl<TaskId, T: KernelTask<TaskId>> Task<TaskId, T> {
    pub fn state(&self) -> TaskState {
        self.state
    }

    pub fn context_handle(&self) -> ContextHandle {
        self.cpu_context_handle
    }

    pub fn task_id(&self) -> TaskId {
        self.kernel.task_id()
    }

    pub fn kernel(&self) -> &T {
        &self.kernel
    }

    pub fn kernel_mut(&mut self) -> &mut T {
        &mut self.kernel
    }

    pub fn suspend(
        &mut self,
        rip: EVAddr,
    ) {
        self.state = TaskState::Yielded(rip);
    }

    pub fn wake_up(&mut self) {
        self.state = match self.state {
            TaskState::Runnable => panic!(),
            TaskState::WakeUp(addr) => TaskState::WakeUp(addr),
            TaskState::Yielded(addr) => TaskState::WakeUp(addr),
        }
    }
}

#[derive(Debug)]
enum TaskEntry<TaskId, T: KernelTask<TaskId>> {
    Deleted(Option<Task<TaskId, T>>),
    Task(Option<Task<TaskId, T>>),
}

impl<TaskId, T: KernelTask<TaskId>> From<Task<TaskId, T>> for TaskEntry<TaskId, T> {
    fn from(value: Task<TaskId, T>) -> Self {
        Self::Task(Some(value))
    }
}

impl<TaskId, T: KernelTask<TaskId>> TaskEntry<TaskId, T> {
    /// Transforms the TaskEntry into a deleted entry. The contained Task is unaffected by this.
    /// The main purpose of this is to signify that the task was deleted, but is still kept
    /// in order to be able to restore a previous snapshot.
    fn delete(&mut self) {
        if let TaskEntry::Task(t) = self {
            *self = TaskEntry::Deleted(Some(t.take().unwrap()))
        }
    }

    fn restore(&mut self) {
        if let TaskEntry::Deleted(t) = self {
            *self = TaskEntry::Task(Some(t.take().unwrap()))
        }
    }

    fn task(&self) -> Option<&Task<TaskId, T>> {
        match self {
            TaskEntry::Deleted(t) => None,
            TaskEntry::Task(t) => t.as_ref(),
        }
    }

    fn task_mut(&mut self) -> Option<&mut Task<TaskId, T>> {
        match self {
            TaskEntry::Deleted(t) => None,
            TaskEntry::Task(t) => t.as_mut(),
        }
    }
}

#[derive(Debug, Default)]
pub struct TaskManager<TaskId, T>
where
    TaskId: Ord + Copy + Clone + Default,
    T: KernelTask<TaskId>,
{
    /// The tasks are stored in a simple vector. Only elements that are newer than the latest
    /// snapshot can be removed from this vector. When a snapshot occurs, each entry in the
    /// vector is also snapshot.
    tasks: Vec<TaskEntry<TaskId, T>>,
    /// This is simply a mapping of `TaskId` to an index into the tasks vector.
    /// This makes it possible to do an expensive lookup once, and then simply access by index.
    task_map: BTreeMap<TaskId, usize>,
    /// The current task is an index into the tasks vector
    current_task: Option<usize>,
    snapshots: Vec<TaskManagerSnapshot<TaskId>>,
}

impl<TaskId, T> TaskManager<TaskId, T>
where
    TaskId: Ord + Copy + Clone + Default,
    T: KernelTask<TaskId>,
{
    pub fn new() -> Self {
        TaskManager {
            tasks: Default::default(),
            task_map: Default::default(),
            current_task: Default::default(),
            snapshots: Default::default(),
        }
    }

    /// Adds the provided task to the currently active tasks.
    /// Returns a reference to the created task object upon success.
    pub(crate) fn new_task(
        &mut self,
        kernel_task: T,
        cpu_context_handle: ContextHandle,
    ) -> Result<&Task<TaskId, T>, CPUError> {
        let task = Task {
            kernel: kernel_task,
            state: TaskState::Runnable,
            cpu_context_handle,
            snapshots: Default::default(),
            _phantom: Default::default(),
        };
        // Insert the mapping. If this doesn't fail we can then insert the actual task at the end
        // of the tasks vector
        let task_index = self.tasks.len();
        self.task_map
            .try_insert(task.kernel.task_id(), task_index)
            .map_err(|_| CPUError::TaskExists)?;
        self.tasks.push(task.into());
        Ok(self
            .tasks
            .last()
            .and_then(TaskEntry::task)
            .expect("We just inserted this"))
    }

    pub fn current_task(&self) -> Option<&Task<TaskId, T>> {
        let current_task = self.current_task?;
        self.tasks.get(current_task).and_then(TaskEntry::task)
    }

    pub fn set_current_task(
        &mut self,
        task_id: TaskId,
    ) {
        let index = self.task_map.get(&task_id).unwrap();
        self.current_task.insert(*index);
    }

    pub fn current_task_mut(&mut self) -> Option<&mut Task<TaskId, T>> {
        let current_task = self.current_task?;
        self.tasks
            .get_mut(current_task)
            .and_then(TaskEntry::task_mut)
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task<TaskId, T>> {
        self.tasks.iter().filter_map(TaskEntry::task)
    }

    pub fn tasks_mut(&mut self) -> impl Iterator<Item = &mut Task<TaskId, T>> {
        self.tasks.iter_mut().filter_map(TaskEntry::task_mut)
    }

    /// Get the task with id `task_id`. Returns `None` if no task with this id exists.
    pub fn task(
        &self,
        task_id: &TaskId,
    ) -> Option<&Task<TaskId, T>> {
        self.task_map
            .get(task_id)
            .and_then(|task_index| self.tasks.get(*task_index).and_then(TaskEntry::task))
    }
}
