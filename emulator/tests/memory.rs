use std::path::Path;

use crate::common::{emulate, TestFailure};

mod common;

#[test]
fn mremap_private_anon() -> Result<(), TestFailure> {
    emulate(Path::new("tests/assets/ubuntu2304"), Path::new("/home/user/mremap_private_anon"))
}
