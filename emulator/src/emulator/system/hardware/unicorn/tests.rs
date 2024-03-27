#![cfg(test)]

use crate::emulator::system::hardware::unicorn::UCWrapper;
use crate::emulator::system::GenericError;
use mogi_system::components::mmu::{
    Permission,
    MMU,
};

#[test]
fn basic_map() -> Result<(), GenericError> {
    let u = UCWrapper::new();
    u.borrow_mut()
        .alloc_physical(0u64.into(), 0x1000, Permission::ALL);
    u.borrow_mut().dealloc_physical(0u64.into(), 0x1000);
    Ok(())
}

#[test]
fn write() -> Result<(), GenericError> {
    let u = UCWrapper::new();
    let data = vec![41, 41, 41];
    u.borrow_mut()
        .alloc_physical(0u64.into(), 0x1000, Permission::ALL)?;
    u.borrow_mut().write_physical(0u64.into(), &data);
    let r = u.borrow_mut().read_physical(0u64.into(), data.len())?;
    assert_eq!(data, r);
    Ok(())
}
