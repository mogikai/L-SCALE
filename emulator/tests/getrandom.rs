use std::path::Path;

use common::{get_sysdir, prepare_binary, setup_test};

use crate::common::{emulate, TestFailure};

mod common;

#[test]
fn test_getrandom() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("getrandom");
    emulate(&get_sysdir(), Path::new("/home/user/getrandom"))
}
