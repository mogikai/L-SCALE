use std::path::Path;

use crate::common::{emulate, setup_logging, TestFailure};

mod common;

#[test]
fn test_getrandom_works() -> Result<(), TestFailure> {
    setup_logging();
    assert!(emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/getrandom")).is_ok());
    Ok(())
}
