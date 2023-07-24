pub mod listener;
pub mod socket;

#[derive(Debug, Default)]
pub struct TCPData {}

impl TCPData {
    pub fn new() -> Self {
        Self {}
    }
}
