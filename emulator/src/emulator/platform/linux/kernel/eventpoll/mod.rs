use std::collections::BTreeMap;

use bitflags::Flags;
use byteorder::ByteOrder;
use num_enum::TryFromPrimitive;

use fuzzyemu_derive::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::CPU;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::mmu::MMU;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::{FD, FileDescriptorData};
use crate::emulator::platform::linux::kernel::eventpoll::events::EPOLL_EVENTS;
use crate::emulator::platform::linux::kernel::{LinuxKernelError, LinuxKernelTask, SyscallResult};
use crate::emulator::platform::linux::kernel::eventpoll::fd_ops::EpollOps;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::memory::rw_interface::{MemoryRead, MemoryWrite};
use crate::emulator::system::memory::MemoryAccessor;

mod events;
mod fd_ops;

impl<Endianness: ByteOrder, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn epoll_create(
        &mut self,
        _: &mut impl CPU,
        size: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn epoll_wait(
        &mut self,
        mmu: &mut impl MMU,
        epfd: FD,
        events: EVAddr, // epoll_event*
        maxevents: u64,
        timeout: u64,
    ) -> SyscallResult {
        // TODO: Need to implement yielding when no fds are ready
        //       Need to advance the clock by fuzzer/callback specified amount
        //       The controller (e.g. fuzzer) of the emulator needs to be able
        //       to specify the elapsed time before an event occurs and also
        //       needs to be able to see which events are asked for, such that
        //       it can react accordingly and produce events. This might be
        //       possible via some kind of event loop between fuzzer and emulator?
        let mut fd_map = self.system.fd_map.borrow_mut();

        let mut epoll = fd_map
            .get_mut(&self.task_id, &epfd)
            .ok_or(LinuxKernelError::EBADF)?;
        let epoll = &mut *epoll;
        let epoll_data = epoll.data.epoll.as_mut().ok_or(LinuxKernelError::EINVAL)?;

        let mut triggered_events = vec![];
        let mut fds_to_clean = vec![];
        for (fd, event) in &epoll_data.interest_list {
            // If we can't find the fd it means it was closed in the meantime. Remove it.
            let file = fd_map.get_mut(&self.task_id, &fd);
            match file {
                None => fds_to_clean.push(*fd),
                Some(mut file) => {
                    let file = &mut *file;
                    let ready_ops = file
                        .ops
                        .poll(&mut file.data)
                        .expect("Only pollable fds should be in interest list");
                    let ready_events = EPOLL_EVENTS::from(ready_ops);
                    if event.events.intersects(ready_events) {
                        if triggered_events.len() < maxevents as usize {
                            triggered_events.push(epoll_event {
                                events: ready_events,
                                data: event.data,
                            });
                        }
                    }
                }
            }
        }

        fds_to_clean.into_iter().for_each(|fd| {
            epoll_data.interest_list.remove(&fd);
        });

        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);

        memory.write(events, triggered_events.as_slice());

        Ok(triggered_events.len() as u64)
    }

    pub fn epoll_ctl(
        &mut self,
        mmu: &mut impl MMU,
        epfd: FD,
        op: u64,
        fd: FD,
        event: EVAddr, // epoll_event*
    ) -> SyscallResult {
        let mut fd_map = self.system.fd_map.borrow_mut();
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);

        let op = EPOLL_OPCODES::try_from_primitive(op).map_err(|_| LinuxKernelError::EINVAL)?;

        if epfd == fd {
            Err(LinuxKernelError::EINVAL)?
        }

        // Check if fd is valid by fetching it
        let mut file_descriptor = fd_map
            .get_mut(&self.task_id, &fd)
            .ok_or(LinuxKernelError::EBADF)?;
        let file_descriptor = &mut *file_descriptor;
        let mut epfd = fd_map
            .get_mut(&self.task_id, &epfd)
            .ok_or(LinuxKernelError::EBADF)?;

        // Is this an epoll fd?
        let epoll = epfd.data.epoll.as_mut().ok_or(LinuxKernelError::EINVAL)?;

        // Can fd be polled?
        if !file_descriptor.ops.supports_poll(&mut file_descriptor.data) {
            Err(LinuxKernelError::EINVAL)?
        }

        match op {
            EPOLL_OPCODES::EPOLL_CTL_ADD => self.epoll_ctl_add(&memory, epoll, &fd, file_descriptor, event),
            EPOLL_OPCODES::EPOLL_CTL_DEL => self.epoll_ctl_del(epoll, &fd),
            EPOLL_OPCODES::EPOLL_CTL_MOD => self.epoll_ctl_mod(&memory, epoll, &fd, event),
        }
    }

    fn epoll_ctl_add(
        &self,
        memory: &MemoryAccessor<Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        file_descriptor: &mut FileDescriptor<FileDescriptorData>,
        event: EVAddr,
    ) -> SyscallResult {
        // TODO: Do loop check (see https://man7.org/linux/man-pages/man2/epoll_ctl.2.html)
        let event: epoll_event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) && file_descriptor.data.epoll.is_some() {
            Err(LinuxKernelError::EINVAL)?
        }

        epoll
            .interest_list
            .try_insert(*fd, event)
            .map_err(|_| LinuxKernelError::EEXIST)?;
        Ok(0)
    }

    fn epoll_ctl_del(
        &self,
        epoll: &mut EpollData,
        fd: &FD,
    ) -> SyscallResult {
        epoll
            .interest_list
            .remove(&fd)
            .ok_or(LinuxKernelError::ENOENT)?;
        Ok(0)
    }

    fn epoll_ctl_mod(
        &self,
        memory: &MemoryAccessor<Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        event: EVAddr,
    ) -> SyscallResult {
        let event: epoll_event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) {
            Err(LinuxKernelError::EINVAL)?
        }

        let current_event = epoll
            .interest_list
            .get_mut(&fd)
            .ok_or(LinuxKernelError::EINVAL)?;

        if current_event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) {
            Err(LinuxKernelError::EINVAL)?
        }

        *current_event = event;
        Ok(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn epoll_pwait(
        &mut self,
        _: &mut impl CPU,
        epfd: u64,
        events: EVAddr, // epoll_event*
        maxevents: u64,
        timeout: u64,
        sigmask: EVAddr, // sigset_t*
        sigsetsize: u64,
    ) -> SyscallResult {
        // TODO: We need to possibly yield to the scheduler here. So we would need to stop emulation and return
        //       control to outside of the syscall handler so we can perform a context switch.
        //       Another possibility would be to simply not yield here and employ some kind of time slice
        //       scheduling, where each thread / process is run for X iterations.
        todo!()
    }

    pub fn epoll_create1(
        &mut self,
        _: &mut impl CPU,
        flags: u64,
    ) -> SyscallResult {
        let mut fd = FileDescriptor::new(Box::new(EpollOps::new()));
        fd.data.epoll = Some(EpollData::new());
        Ok(self
            .system
            .fd_map
            .borrow_mut()
            .insert(&self.task_id, fd)?
            .into())
    }
}

#[derive(Debug)]
pub struct EpollData {
    interest_list: BTreeMap<FD, epoll_event>,
}

impl EpollData {
    fn new() -> Self {
        EpollData {
            interest_list: Default::default(),
        }
    }
}

#[derive(Debug, Decode, Encode, RequiredMem, Copy, Clone)]
#[repr(C)]
#[allow(non_camel_case_types)]
struct epoll_event {
    events: EPOLL_EVENTS,
    data: u64,
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive)]
#[repr(u64)]
pub enum EPOLL_OPCODES {
    EPOLL_CTL_ADD = 1,
    EPOLL_CTL_DEL = 2,
    EPOLL_CTL_MOD = 3,
}
