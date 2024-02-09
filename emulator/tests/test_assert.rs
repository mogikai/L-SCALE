//! Tests whether the assert mechanism in emulated test targets works correctly.
//! An assert(0) must cause a sigabrt and any other assert call must result in
//! normal program termination.
use std::path::Path;

use crate::common::{emulate, get_sysdir, prepare_binary, setup_logging, TestFailure};

mod common;

#[test]
fn test_assert_works() -> Result<(), TestFailure> {
    setup_logging();
    let d = get_sysdir();
    prepare_binary("assert_false");
    prepare_binary("assert_true");
    assert!(emulate(&d, Path::new("/home/user/assert_false")).is_err());
    assert!(emulate(&d, Path::new("/home/user/assert_true")).is_ok());
    Ok(())
}
