use std::path::Path;

use crate::common::{emulate, setup_logging, TestFailure};

mod common;

#[test]
fn mremap_private_anon() -> Result<(), TestFailure> {
    setup_logging();
    emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/mremap_private_anon"))
}
