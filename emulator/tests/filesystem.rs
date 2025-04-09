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
fn fstatat() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("fstatat")?;
    emulate(&get_sysdir(), Path::new("/home/user/fstatat"))
}

#[test]
fn chdir() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("chdir")?;
    emulate(&get_sysdir(), Path::new("/home/user/chdir"))
}
