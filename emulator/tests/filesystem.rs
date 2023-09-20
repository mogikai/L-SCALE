use std::path::Path;

use crate::common::{emulate, TestFailure};

mod common;

#[test]
fn fstatat() -> Result<(), TestFailure> {
    emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/fstatat"))
}
