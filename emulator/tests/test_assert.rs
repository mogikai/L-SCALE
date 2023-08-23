//! Tests whether the assert mechanism in emulated test targets works correctly.
//! An assert(0) must cause a sigabrt and any other assert call must result in
//! normal program termination.
use std::path::Path;

use crate::common::{emulate, setup_logging, TestFailure};

mod common;

#[test]
fn test_assert_works() -> Result<(), TestFailure> {
    setup_logging();
    assert!(emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/assert_false")).is_err());
    assert!(emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/assert_true")).is_ok());
    Ok(())
}
