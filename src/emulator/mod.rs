pub mod system;
pub mod abstractions;
pub mod platform;

pub trait Snapshot {
    fn snapshot(&mut self);
    fn restore(&mut self);
}