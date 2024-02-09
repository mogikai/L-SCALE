use std::path::Path;

use crate::common::{emulate, get_sysdir, prepare_binary, setup_logging, TestFailure};

mod common;

#[test]
fn mremap_private_anon() -> Result<(), TestFailure> {
    setup_logging();
    prepare_binary("mremap_private_anon");
    emulate(&get_sysdir(), Path::new("/home/user/mremap_private_anon"))
}
