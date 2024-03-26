#![cfg(test)]

use crate::emulator::system::memory::tlb::*;

#[test]
fn basic_map_succeeds() {
    let mut tlb = Tlb::new();
    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_ok());
}

#[test]
fn unaligned_map_fails() {
    let mut tlb = Tlb::new();
    let result =
        tlb.try_map(0x2020u64.into(), 0x1000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_err());

    let result =
        tlb.try_map(0x2000u64.into(), 0x1020u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_err());

    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x1020, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_err());
}

#[test]
fn double_map_fails() {
    let mut tlb = Tlb::new();
    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_ok());

    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_err());
}

#[test]
fn overlapping_map_fails() {
    let mut tlb = Tlb::new();
    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x4000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_ok());

    let result =
        tlb.try_map(0x4000u64.into(), 0x8000u64.into(), 0x4000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_err());
}

#[test]
fn adjacent_mapping_succeeds() {
    let mut tlb = Tlb::new();
    let result =
        tlb.try_map(0x2000u64.into(), 0x1000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_ok());

    let result =
        tlb.try_map(0x3000u64.into(), 0x8000u64.into(), 0x1000, Permission::ALL, MappingType::PrivateAnonymous, None);
    assert!(result.is_ok());
}
