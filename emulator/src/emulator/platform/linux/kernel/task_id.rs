#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, Ord, PartialOrd)]
pub struct TaskId {
    pub pid: u32,
    pub tid: u32,
}

impl TaskId {
    pub fn new(
        pid: u32,
        tid: u32,
    ) -> Self {
        Self { pid, tid }
    }
}