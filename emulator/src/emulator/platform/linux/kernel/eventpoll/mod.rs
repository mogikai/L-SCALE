use std::collections::BTreeMap;
use std::fmt::{
    Debug,
    Formatter,
};

use bitflags::Flags;
use byteorder::ByteOrder;
use imbl::OrdMap;
use num_enum::TryFromPrimitive;
use tracing::trace;

use mogi_derive::{
    Decode,
    Encode,
    RequiredMem,
    unfinished,
};
use mogi_system::components::cpu::CPU;
use mogi_utilities::addresses::EVAddr;

use crate::emulator::abstractions::file_descriptors::{
    FileDescriptorOps,
    FileOperationsReady,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::{
    FD,
    FileDescriptorData,
};
use crate::emulator::platform::linux::kernel::error::SyscallError;
use crate::emulator::platform::linux::kernel::eventpoll::events::EPOLL_EVENTS;
use crate::emulator::platform::linux::kernel::eventpoll::fd_ops::EpollOps;
use crate::emulator::platform::linux::kernel::{
    LinuxKernelError,
    LinuxKernelTask,
    SyscallResult,
    sys_component_mut,
};
use crate::emulator::system::fd_map::{
    FileDescriptor,
    TemporaryRemoveGuard,
};
use crate::emulator::system::memory::MemoryAccessor;
use crate::emulator::system::memory::rw_interface::{
    MemoryRead,
    MemoryWrite,
};
use mogi_system::components::mmu::MMU;

mod events;
mod fd_ops;

impl<Endianness, Arch, CpuT, MmuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, MmuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU,
    MmuT: MMU,
    TcpT: TCPSystem,
{
    pub fn epoll_create(
        &mut self,
        size: u64,
    ) -> SyscallResult {
        todo!()
    }

    // #[unfinished]
    pub fn epoll_wait(
        &mut self,
        epfd: FD,
        events: EVAddr<epoll_event>,
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
        let mut fd_map = sys_component_mut!(self, fd_map);

        let mut epoll = fd_map
            .remove_temporarily(&self.task_id, &epfd)
            .ok_or(LinuxKernelError::EBADF)?;
        trace!(
            "Epoll interest list: {:?}",
            epoll
                .data
                .epoll
                .as_mut()
                .ok_or(LinuxKernelError::EINVAL)?
                .interest_list
        );

        let mut triggered_events = vec![];
        epoll.with_fd_map(|fd_map, epoll| {
            let mut fds_to_clean = vec![];
            let epoll_data = epoll.data.epoll.as_mut().ok_or(LinuxKernelError::EINVAL)?;
            for (fd, event) in &epoll_data.interest_list {
                // If we can't find the fd it means it was closed in the meantime. Remove it.
                let file = fd_map.get_mut(&self.task_id, fd);
                match file {
                    None => fds_to_clean.push(*fd),
                    Some(mut file) => {
                        let ready_ops = file
                            .poll(FileOperationsReady::from(event.events))
                            .expect("Only pollable fds should be in interest list");
                        let ready_events = EPOLL_EVENTS::from(ready_ops);
                        if event.events.intersects(ready_events) && triggered_events.len() < maxevents as usize {
                            let event = epoll_event {
                                events: ready_events,
                                data: event.data,
                            };
                            trace!("Adding triggered event: {event:?}");
                            triggered_events.push(event);
                        }
                    }
                }
            }

            fds_to_clean.into_iter().for_each(|fd| {
                epoll_data.interest_list.remove(&fd);
            });
            SyscallResult::Ok(0)
        });

        if triggered_events.is_empty() && timeout != 0 {
            Err(SyscallError::Yield)?
        }

        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut mmu, self.task_id);

        memory.write_slice(events, triggered_events.as_slice())?;

        Ok(triggered_events.len() as u64)
    }

    pub fn epoll_ctl(
        &mut self,
        epfd: FD,
        op: u64,
        fd: FD,
        event: EVAddr<epoll_event>,
    ) -> SyscallResult {
        let mut fd_map = sys_component_mut!(self, fd_map);
        let mut mmu = sys_component_mut!(self, mmu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut mmu, self.task_id);

        let op = EPOLL_OPCODES::try_from_primitive(op).map_err(|_| LinuxKernelError::EINVAL)?;

        if epfd == fd {
            Err(LinuxKernelError::EINVAL)?
        }

        let mut epfd = fd_map
            .remove_temporarily(&self.task_id, &epfd)
            .ok_or(LinuxKernelError::EBADF)?;
        epfd.with_fd_map(|fd_map, epfd| {
            // Check if fd is valid by fetching it
            let mut file_descriptor = fd_map
                .get_mut(&self.task_id, &fd)
                .ok_or(LinuxKernelError::EBADF)?;
            let file_descriptor = &mut *file_descriptor;

            // Is this an epoll fd?
            let epoll = epfd.data.epoll.as_mut().ok_or(LinuxKernelError::EINVAL)?;

            // Can fd be polled?
            if !file_descriptor.supports_poll() {
                Err(LinuxKernelError::EINVAL)?
            }

            match op {
                EPOLL_OPCODES::EPOLL_CTL_ADD => self.epoll_ctl_add(&mut memory, epoll, &fd, file_descriptor, event),
                EPOLL_OPCODES::EPOLL_CTL_DEL => self.epoll_ctl_del(epoll, &fd),
                EPOLL_OPCODES::EPOLL_CTL_MOD => self.epoll_ctl_mod(&mut memory, epoll, &fd, event),
            }
        })
    }

    #[allow(clippy::needless_pass_by_ref_mut)] // In the future mutable access will be needed
    fn epoll_ctl_add(
        &self,
        memory: &mut MemoryAccessor<'_, Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        file_descriptor: &mut FileDescriptor<FileDescriptorData>,
        event: EVAddr<epoll_event>,
    ) -> SyscallResult {
        // TODO: Do loop check (see https://man7.org/linux/man-pages/man2/epoll_ctl.2.html)
        let event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) && file_descriptor.data.epoll.is_some() {
            Err(LinuxKernelError::EINVAL)?
        }
        if epoll.interest_list.contains_key(fd) {
            Err(LinuxKernelError::EEXIST)?;
        }
        epoll.interest_list.insert(*fd, event);
        Ok(0)
    }

    fn epoll_ctl_del(
        &self,
        epoll: &mut EpollData,
        fd: &FD,
    ) -> SyscallResult {
        epoll
            .interest_list
            .remove(fd)
            .ok_or(LinuxKernelError::ENOENT)?;
        Ok(0)
    }

    fn epoll_ctl_mod(
        &self,
        memory: &mut MemoryAccessor<'_, Endianness, impl MMU + Sized>,
        epoll: &mut EpollData,
        fd: &FD,
        event: EVAddr<epoll_event>,
    ) -> SyscallResult {
        let event: epoll_event = memory.read(event)?;

        if event.events.contains(EPOLL_EVENTS::EPOLLEXCLUSIVE) {
            Err(LinuxKernelError::EINVAL)?
        }

        let current_event = epoll
            .interest_list
            .get_mut(fd)
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
        flags: u64,
    ) -> SyscallResult {
        let mut fd = FileDescriptor::new(Box::new(EpollOps::new()));
        fd.data.epoll = Some(EpollData::new());
        Ok(sys_component_mut!(self, fd_map)
            .insert(&self.task_id, fd)?
            .into())
    }
}

#[derive(Debug, Clone)]
pub struct EpollData {
    interest_list: OrdMap<FD, epoll_event>,
}

impl EpollData {
    fn new() -> Self {
        EpollData {
            interest_list: Default::default(),
        }
    }
}

#[derive(Decode, Encode, RequiredMem, Copy, Clone)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub(crate) struct epoll_event {
    events: EPOLL_EVENTS,
    data: u64,
}

impl Debug for epoll_event {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("epoll_event")
            .field("events", &self.events)
            .field("data", &format!("{:#018x}", &self.data))
            .finish()
    }
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive)]
#[repr(u64)]
pub enum EPOLL_OPCODES {
    EPOLL_CTL_ADD = 1,
    EPOLL_CTL_DEL = 2,
    EPOLL_CTL_MOD = 3,
}
