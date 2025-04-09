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
fn mremap_private_anon() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("mremap_private_anon")?;
    emulate(&get_sysdir(), Path::new("/home/user/mremap_private_anon"))
}

#[test]
fn mmap() -> Result<(), TestFailure> {
    setup_test();
    prepare_binary("mmap")?;
    emulate(&get_sysdir(), Path::new("/home/user/mmap"))
}
