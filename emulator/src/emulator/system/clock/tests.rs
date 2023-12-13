#![cfg(test)]

use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::*;
use std::time::Duration;

#[test]
fn test_snapshot() {
    let mut c = Clock::new(Duration::ZERO);
    assert!(c.take_snapshot().is_ok());
    c.do_timestep(1);
    let d1 = c.time();
    assert_ne!(d1, Duration::ZERO);
    assert!(c.restore_snapshot().is_ok());
    let d2 = c.time();
    assert_eq!(d2, Duration::ZERO);
    c.do_timestep(1);
    let d2 = c.time();
    assert_eq!(d1, d2);
    assert!(c.discard_current_and_restore_previous_snapshot().is_ok());
    let d = c.time();
    assert_eq!(d, Duration::ZERO);
    assert!(c.discard_current_and_restore_previous_snapshot().is_err());
}
