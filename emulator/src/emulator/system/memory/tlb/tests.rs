#![cfg(test)]

use crate::emulator::system::memory::tlb::*;

#[test]
fn basic_map_succeeds() {
    let mut tlb = TLB::new();
    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_ok());
}

#[test]
fn unaligned_map_fails() {
    let mut tlb = TLB::new();
    let result = tlb.try_map(
        0x2020.into(),
        0x1000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_err());

    let result = tlb.try_map(
        0x2000.into(),
        0x1020.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_err());

    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x1020,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_err());
}

#[test]
fn double_map_fails() {
    let mut tlb = TLB::new();
    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_ok());

    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_err());
}

#[test]
fn overlapping_map_fails() {
    let mut tlb = TLB::new();
    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x4000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_ok());

    let result = tlb.try_map(
        0x4000.into(),
        0x8000.into(),
        0x4000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_err());
}

#[test]
fn adjacent_mapping_succeeds() {
    let mut tlb = TLB::new();
    let result = tlb.try_map(
        0x2000.into(),
        0x1000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_ok());

    let result = tlb.try_map(
        0x3000.into(),
        0x8000.into(),
        0x1000,
        Permission::ALL,
        MappingType::PrivateAnonymous,
    );
    assert!(result.is_ok());
}
