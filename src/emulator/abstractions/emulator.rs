use std::cell::{Ref, RefCell, RefMut};
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;

use unicorn_engine::{InsnSysX86, RegisterX86, Unicorn};
use unicorn_engine::unicorn_const::{Arch, Mode, Permission, uc_error};

#[derive(Debug)]
pub enum StopReason {
    Unknown
}

pub trait Emulator {
    fn run(&mut self) -> Result<(), StopReason>;
}