//! Tests whether the assert mechanism in emulated test targets works correctly.
//! An assert(0) must cause a sigabrt and any other assert call must result in
//! normal program termination.
use std::path::Path;

use crate::common::{
    TestFailure,
    emulate,
    get_sysdir,
    prepare_binary,
    setup_test,
};

mod common;

#[test]
fn test_assert_works() -> Result<(), TestFailure> {
    setup_test();
    let d = get_sysdir();
    prepare_binary("assert_false")?;
    prepare_binary("assert_true")?;
    // TODO: Reenable once done adapting to new cpu emulator.
    // assert!(emulate(&d, Path::new("/home/user/assert_false")).is_err());
    assert!(emulate(&d, Path::new("/home/user/assert_false")).is_err());
    assert!(emulate(&d, Path::new("/home/user/assert_true")).is_ok());
    Ok(())
}
