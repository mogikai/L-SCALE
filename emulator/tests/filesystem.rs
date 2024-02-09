use std::path::Path;

use crate::common::{emulate, get_sysdir, prepare_binary, setup_logging, TestFailure};

mod common;

#[test]
fn fstatat() -> Result<(), TestFailure> {
    setup_logging();
    prepare_binary("fstatat");
    emulate(&get_sysdir(), Path::new("/home/user/fstatat"))
}
