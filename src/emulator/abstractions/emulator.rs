#[derive(Debug)]
pub enum StopReason {
    Unknown
}

pub trait Emulator {
    fn run(&mut self) -> Result<(), StopReason>;
}