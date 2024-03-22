#![cfg(test)]

use crate::emulator::abstractions::mmu::{
    Permission,
    MMU,
};
use crate::emulator::system::hardware::unicorn::UCWrapper;
use crate::emulator::system::GenericError;

#[test]
fn basic_map() -> Result<(), GenericError> {
    let u = UCWrapper::new();
    u.borrow_mut().mem_map(0.into(), 0x1000, Permission::ALL);
    u.borrow_mut().mem_unmap(0.into(), 0x1000);
    Ok(())
}

#[test]
fn write() -> Result<(), GenericError> {
    let u = UCWrapper::new();
    let data = vec![41, 41, 41];
    u.borrow_mut().mem_map(0.into(), 0x1000, Permission::ALL)?;
    u.borrow_mut().mem_write_bytes(0.into(), &data);
    let r = u.borrow_mut().mem_read_bytes(0.into(), data.len())?;
    assert_eq!(data, r);
    Ok(())
}
