#![cfg(test)]

use crate::emulator::system::clock::*;
use crate::emulator::abstractions::restore::Restorable;
use std::time::Duration;

#[test]
fn test_snapshot() {
    let mut c = Clock::new(Duration::ZERO);
    assert!(c.snapshot().is_ok());
    c.do_timestep(1);
    let d1 = c.time();
    assert!(d1 != Duration::ZERO);
    assert!(c.restore().is_ok());
    let d2 = c.time();
    assert!(d2 == Duration::ZERO);
    c.do_timestep(1);
    let d2 = c.time();
    assert!(d1 == d2);
    assert!(c.restore_and_discard().is_ok());
    let d = c.time();
    assert!(d == Duration::ZERO);
    assert!(c.restore_and_discard().is_err());
}
