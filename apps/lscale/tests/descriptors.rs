use std::path::Path;

use common::get_sysdir;

use crate::common::{
    TestFailure,
    emulate,
    prepare_binary,
    setup_test,
};

mod common;

#[test]
fn dup() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("dup")?;
    emulate(&get_sysdir(), Path::new("/home/user/dup"))
}
