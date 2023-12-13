#![cfg(test)]

use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::system::clock::*;
use std::time::Duration;

#[test]
fn test_snapshot() -> Result<(), GenericError> {
    let mut c = Clock::new(Duration::ZERO);
    c.take_snapshot()?;

    c.do_timestep(1);
    let d1 = c.time();
    assert_ne!(d1, Duration::ZERO);
    c.restore_snapshot()?;

    let d2 = c.time();
    assert_eq!(d2, Duration::ZERO);
    c.do_timestep(1);
    c.take_snapshot()?;

    let d2 = c.time();
    assert_eq!(d1, d2);
    c.discard_current_and_restore_previous_snapshot()?;

    let d = c.time();
    assert_eq!(d, Duration::ZERO);
    assert!(c.discard_current_and_restore_previous_snapshot().is_err());
    Ok(())
}
