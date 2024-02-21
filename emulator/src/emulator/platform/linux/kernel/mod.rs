use std::cell::RefCell;
use std::fmt::{
    Debug,
    Formatter,
};
use std::marker::PhantomData;
use std::path::{
    Path,
    PathBuf,
};
use std::rc::Rc;

use byteorder::ByteOrder;
use goblin::elf::{
    dynamic,
    program_header,
    Elf,
};
use num_enum::FromPrimitive;
use tracing::{
    error,
    info,
    trace,
    warn,
    debug,
};

use error::SyscallError;
use fuzzyemu_derive::{
    Decode,
    Encode,
    RequiredMem,
};

use crate::emulator::abstractions::cpu::address::{
    EVAddr,
    EVA_NULL,
};
use crate::emulator::abstractions::cpu::{
    RegisterX86,
    CPU,
};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::filesystem::FileId;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::kernel_task::KernelTask;
use crate::emulator::abstractions::mmu::encoding::{
    Decode,
    Encode,
    RequiredMem,
};
use crate::emulator::abstractions::mmu::{
    Permission,
    MMU,
};
use crate::emulator::abstractions::network::tcp_system::TCPSystem;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::macros::push_stack;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::{
    FileDescriptorData,
    FD,
};
use crate::emulator::platform::linux::kernel::auxv::AUXVType;
pub use crate::emulator::platform::linux::kernel::error::LinuxKernelError;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::signalfd::SignalFDOps;
use crate::emulator::platform::linux::signals::{
    SignalHandler,
    SignalMaskFlags,
};
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::system::default_handles;
use crate::emulator::system::fd_map::{
    FileDescriptor,
    InternalFD,
};
use crate::emulator::system::memory::rw_interface::{
    DirectMemoryWrite,
    MemoryRead,
    MemoryWrite,
};

pub(crate) mod auxv;
pub(crate) mod error;
pub(crate) mod eventpoll;
pub(crate) mod fs;
pub(crate) mod memory;
pub(crate) mod net;
pub mod select;
pub(crate) mod socket;
pub mod task_id;

macro_rules! sys_component {
    ($self:ident, $component:ident) => {
        $self.system.$component.borrow()
    };
}

macro_rules! sys_component_mut {
    ($self:ident, $component:ident) => {
        $self.system.$component.borrow_mut()
    };
}

use crate::emulator::platform::linux::kernel::fs::FileData;
pub(super) use {
    sys_component,
    sys_component_mut,
};

/// The kernel task object does not contain any state.
/// Any state must be stored in the system struct that
/// is available to all tasks.
/// TODO: Currently the task is not truly immutable.
///       However, it would be nice to have, and we
///       don't have to change much to achieve
///       immutability.
pub struct LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    system: LinuxSystem<Endianness, CpuT, TcpT>,
    clear_child_tid: Option<EVAddr>,
    robust_list_head: Option<EVAddr>,
    signalhandler: SignalHandler<Arch::SignalFlags>,
    task_id: TaskId,
    program_info: Option<LoadInfo>,
    interpreter_info: Option<LoadInfo>,
    _arch: PhantomData<Arch>,
}

impl<Endianness, Arch, CpuT, TcpT> Debug for LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "TODO")
    }
}

impl<Endianness, Arch, CpuT, TcpT> KernelTask<TaskId> for LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn task_id(&self) -> TaskId {
        self.task_id
    }
    fn entrypoint(&self) -> Option<EVAddr> {
        self.program_info
            .as_ref()
            .map(|load_info| load_info.entrypoint)
    }

    fn load_base(&self) -> Option<EVAddr> {
        self.program_info
            .as_ref()
            .map(|load_info| load_info.base_address)
    }
}

type SyscallResult = Result<u64, SyscallError>;

/// The LoadInfo struct contains some information about a loaded program
#[derive(Debug)]
struct LoadInfo {
    /// If the interpreter is None, then the loaded file was the interpreter
    interpreter_path: Option<PathBuf>,
    /// If the program contains a phdr section this is the address to that section.
    program_header_address: Option<EVAddr>,
    /// The base address to where this executable was loaded
    base_address: EVAddr,
    /// The entrypoint of the executable
    entrypoint: EVAddr,
    /// The number of program headers
    program_header_num: u64,
    /// The size of a program header entry
    program_header_entry_size: u64,
}

impl<Endianness, Arch, CpuT, TcpT> LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder + 'static,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    pub fn new(
        system: LinuxSystem<Endianness, CpuT, TcpT>,
        pid: u32,
        tid: u32,
    ) -> Self {
        LinuxKernelTask {
            system,
            clear_child_tid: None,
            robust_list_head: None,
            signalhandler: SignalHandler::new(),
            task_id: TaskId::new(pid, tid),
            program_info: None,
            interpreter_info: None,
            _arch: PhantomData,
        }
    }

    pub fn load_binary(
        &mut self,
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), LinuxKernelError> {
        {
            let mut cpu = sys_component_mut!(self, cpu);
            sys_component_mut!(self, memory).new_process(&mut *cpu, self.task_id)?;
        }

        trace!("Loading program");
        self.program_info = Some(self.load_elf(binary_path, 0x0000_c000_0000_0000.into())?);

        trace!("Loading interpreter");
        // Load interpreter
        let interpreter_load_info = self.load_elf(
            self.program_info
                .as_ref()
                .expect("Set above")
                .interpreter_path
                .clone()
                .expect("Currently we only support loading interpreted programs")
                .as_path(),
            0x0000_b000_0000_0000.into(),
        )?;

        self.program_info.as_mut().expect("Set above").base_address = interpreter_load_info.base_address;

        self.initstack(binary_path, binary_args, binary_env)?;

        // Write the interpreter entrypoint into the instruction pointer so we can start execution.
        sys_component_mut!(self, cpu).reg_write(RegisterX86::RIP.into(), interpreter_load_info.entrypoint.into())?;
        Ok(())
    }

    fn load_elf(
        &mut self,
        path: &Path,
        mut base_address: EVAddr,
    ) -> Result<LoadInfo, LinuxKernelError> {
        let fd = self.open_internal(path)?;
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut fd_map = sys_component_mut!(self, fd_map);
        let elf_data = {
            let mut elf_file = fd_map
                .get_mut_internal(&self.task_id, &fd)
                .ok_or(LinuxKernelError::ENOENT)?;
            trace!("Loading elf file {elf_file:?}");
            let elf_file = &mut *elf_file;
            elf_file.pread(0, u64::MAX)?
        };

        let elf = Elf::parse(&elf_data).unwrap();

        if !elf.is_lib {
            if let Some(ref dynamic) = elf.dynamic {
                let dyn_info = &dynamic.info;
                if dyn_info.flags_1 & dynamic::DF_1_PIE == 0 {
                    trace!("not pie");
                    base_address = EVA_NULL;
                }
            } else {
                base_address = EVA_NULL;
            }
        }
        debug!("base address: {:?}", base_address);
        for ph in &elf.program_headers {
            if ph.p_type == program_header::PT_LOAD {
                let perms = getperms(ph);
                if ph.p_offset + ph.p_filesz > elf_data.len().try_into().unwrap() {
                    return Err(LinuxKernelError::ENOEXEC);
                }
                let vm_range =
                    base_address + EVAddr::from(ph.p_vaddr)..(base_address + EVAddr::from(ph.p_vaddr) + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                memory
                    .map_file(&mut *cpu, &mut fd_map, &self.task_id, vm_range, &fd, ph.file_range(), perms)
                    .unwrap();
            }
        }

        let interpreter_path = elf.interpreter.map(PathBuf::from);
        let program_header_address = elf
            .program_headers
            .iter()
            .find(|ph| ph.p_type == program_header::PT_PHDR)
            .map(|ph| base_address + ph.p_vaddr);
        let entrypoint = base_address + EVAddr::from(elf.entry);

        Ok(LoadInfo {
            interpreter_path,
            program_header_address,
            base_address,
            entrypoint,
            program_header_num: elf.header.e_phnum as u64,
            program_header_entry_size: elf.header.e_phentsize as u64,
        })
    }

    fn initstack(
        &mut self,
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), LinuxKernelError> {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut argv_strings = vec![binary_path.to_str().unwrap()];
        argv_strings.extend_from_slice(binary_args);

        let stack_base = memory.stack_base(self.task_id).expect("no process found");
        info!("Initializing stack");
        cpu.reg_write(RegisterX86::RSP.into(), stack_base.into())
            .expect("failed to write RSP");
        let mut aux: Vec<(u64, u64)> = Vec::new();
        push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, 0u64, RegisterX86::RSP.into())?; // Stack end marker

        // Push env strings. Pointers are saved in a null terminated env array.
        let env = {
            let mut env = Vec::<u64>::new();
            for &env_str in binary_env {
                env.push(push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, env_str, RegisterX86::RSP.into())?);
            }
            env.push(0); // End of array nullptr
            env
        };

        // Push argv strings. Pointers are saved in a null terminated argv array, prefixed with the length (argc).
        let argv = {
            let mut argv = Vec::<u64>::new();
            argv.push(argv_strings.len() as u64);
            for arg in argv_strings {
                argv.push(push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, arg, RegisterX86::RSP.into())?);
            }
            argv.push(0); // End of array nullptr
            argv
        };

        aux.push((AUXVType::Pagesz as u64, 4096));
        aux.push((AUXVType::HwCap as u64, 0x178bfbff));
        aux.push((AUXVType::Uid as u64, 1000));
        aux.push((AUXVType::Gid as u64, 1000));
        aux.push((AUXVType::EUid as u64, 1000));
        aux.push((AUXVType::EGid as u64, 1000));
        aux.push((
            AUXVType::Platform as u64,
            push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, "x86_64", RegisterX86::RSP.into())?,
        ));
        let program_info = self
            .program_info
            .as_ref()
            .expect("Load is always called before initstack");
        trace!("AUXVType::Entry: {:#?}", program_info.entrypoint);
        aux.push((AUXVType::Entry as u64, program_info.entrypoint.into()));
        trace!(
            "AUXVType::Phdr: {:#?}",
            program_info
                .program_header_address
                .ok_or(LinuxKernelError::EINVAL)?
        );
        aux.push((
            AUXVType::Phdr as u64,
            program_info
                .program_header_address
                .ok_or(LinuxKernelError::EINVAL)?
                .into(),
        ));
        trace!("AUXVType::Phent: {:#?}", program_info.program_header_entry_size);
        aux.push((AUXVType::Phent as u64, program_info.program_header_entry_size));
        trace!("AUXVType::Phnum: {:#?}", program_info.program_header_num);
        aux.push((AUXVType::Phnum as u64, program_info.program_header_num));
        trace!("AUXVType::Base: {:#?}", program_info.base_address);
        aux.push((AUXVType::Base as u64, program_info.base_address.into()));
        aux.push((AUXVType::Flags as u64, 0));
        // TODO: map random data to a more appropriate place and provide an api to set the random data
        memory
            .map_data(
                &mut *cpu,
                self.task_id,
                0xd00000000000.into()..0xd00000000100.into(),
                [0; 16].as_slice(),
                Permission::READ,
            )
            .unwrap();
        aux.push((AUXVType::Random as u64, 0xd00000000000));
        // TODO: Make this configurable?
        aux.push((AUXVType::Secure as u64, 0));
        // AUXV end marker
        aux.push((0, 0));

        // align stack
        let rsp = cpu
            .reg_read(RegisterX86::RSP.into())
            .expect("error can't read RSP");
        if (rsp & !0xf) != rsp {
            cpu.reg_write(RegisterX86::RSP.into(), rsp & !0xf).unwrap();
        }

        push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, &aux, RegisterX86::RSP.into())?;
        push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, &env, RegisterX86::RSP.into())?;
        push_stack!(memory.accessor(&mut cpu, self.task_id), cpu, &argv, RegisterX86::RSP.into())?;

        let rsp = EVAddr::from(cpu.reg_read(RegisterX86::RSP.into())?);

        Ok(())
    }

    pub fn add_stdio_handles(&mut self) {
        let mut file_descriptor_map = sys_component_mut!(self, fd_map);
        let mut read_only = FileDescriptor::new(Box::new(default_handles::ReadOnly::new()));
        // TODO: Dont use pseudo file. Instead use actual filesystem backed input
        read_only.data.file = Some(FileData::new(self.system.filesystem.clone(), FileId::default()));
        file_descriptor_map.insert_at(&self.task_id, 0.into(), read_only);
        file_descriptor_map.insert_at(
            &self.task_id,
            1.into(),
            FileDescriptor::new(Box::new(default_handles::WriteOnly::new())),
        );
        file_descriptor_map.insert_at(
            &self.task_id,
            2.into(),
            FileDescriptor::new(Box::new(default_handles::WriteOnly::new())),
        );
    }

    pub fn brk(
        &mut self,
        addr: EVAddr,
    ) -> SyscallResult {
        let mut memory = sys_component_mut!(self, memory);
        if addr != EVA_NULL {
            memory
                .realloc_heap(self.task_id, addr)
                .inspect_err(|e| warn!("Error during brk: {e:?}"));
        }
        Ok(memory.heap_end(self.task_id)?.into())
    }

    pub fn arch_prctl(
        &mut self,
        code: u64,
        addr: EVAddr,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let memory = memory.accessor(&mut cpu, self.task_id);
        match code {
            0x1001 => {
                //ARCH_SET_GS
                Err(LinuxKernelError::EINVAL.into())
            }
            0x1002 => {
                //ARCH_SET_FS
                let fsbytes = memory.read_bytes(addr, 8).unwrap();
                let fs = Endianness::read_u64(&fsbytes);
                cpu.reg_write(RegisterX86::FS_BASE.into(), fs).unwrap();
                Ok(0)
            }
            0x1003 => Err(LinuxKernelError::EINVAL.into()), //ARCH_GET_FS
            0x1004 => Err(LinuxKernelError::EINVAL.into()), //ARCH_GET_GS
            _ => Err(LinuxKernelError::EINVAL.into()),
        }
    }

    pub fn gettid(&mut self) -> SyscallResult {
        // TODO: Handle tid separately once threads are implemented
        Ok(self.task_id.tid.into())
    }

    pub fn time(
        &mut self,
        tloc: EVAddr,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write_direct(tloc, &(sys_component!(self, clock).time().as_secs() as i64))?;
        Ok(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn futex(
        &mut self,
        uaddr: EVAddr,
        futex_op: u64,
        val: u64,
        timeout: EVAddr,
        uaddr2: EVAddr,
        val3: u64,
    ) -> SyscallResult {
        // FIXME TODO
        error!("Futex syscall not implemented. Currently only returning 0");
        Ok(0)
    }

    pub fn clock_gettime(
        &mut self,
        clk_id: u64,
        tp: EVAddr,
    ) -> SyscallResult {
        let time = sys_component!(self, clock).time();
        let ts = Timespec {
            tv_sec: time.as_secs() as i64,
            tv_nsec: time.subsec_nanos() as i64,
        };
        let mut cpu = sys_component_mut!(self, cpu);
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write(tp, &ts)
            .unwrap();
        Ok(0)
    }

    pub fn getpid(&mut self) -> SyscallResult {
        // TODO: Implement multi threading / processing
        Ok(self.task_id.pid.into())
    }

    pub fn execve(
        &mut self,
        filename: EVAddr,
        argv: EVAddr,
        envp: EVAddr,
    ) -> SyscallResult {
        // TODO: Stop emulation and replace running process with different process.
        Ok(0)
    }

    pub fn uname(
        &mut self,
        buf: EVAddr,
    ) -> SyscallResult {
        //TODO check if address is mapped and has space for struct new_utsname
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
        memory.write_direct(buf, "Linux")?;
        memory.write_direct(buf + 65, "ILT792")?;
        memory.write_direct(buf + 2 * 65, "5.4.0-110-generic")?;
        memory.write_direct(buf + 3 * 65, "#124-Ubuntu SMP Thu Apr 14 19:46:19 UTC 2022")?;
        memory.write_direct(buf + 4 * 65, "x86_64")?;
        memory.write_direct(buf + 5 * 65, "(none)")?;
        Ok(0)
    }

    pub fn fcntl(
        &mut self,
        fd: FD,
        cmd: u64,
        arg: u64,
    ) -> SyscallResult {
        // TODO: Implement correctly
        error!("FCNTL not implemented fully yet");
        sys_component_mut!(self, fd_map).get(&self.task_id, &fd);
        Ok(2)
    }

    pub fn gettimeofday(
        &mut self,
        tv: EVAddr,
        tz: EVAddr,
    ) -> SyscallResult {
        if tv == EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }
        if tz != EVA_NULL {
            todo!("Timezone not implemented yet")
        }
        let time = sys_component!(self, clock).time();
        let timeval = Timeval {
            tv_sec: time.as_secs() as i64,
            tv_usec: time.subsec_micros() as i64,
        };
        let mut cpu = sys_component_mut!(self, cpu);
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write(tv, &timeval)?;
        Ok(0)
    }

    pub fn access(
        &mut self,
        _filename: u64,
        _mode: u64,
    ) -> SyscallResult {
        Err(LinuxKernelError::EINVAL.into())
    }

    pub fn set_tid_address(
        &mut self,
        tidptr: EVAddr,
    ) -> SyscallResult {
        /* todo NULL check */
        self.clear_child_tid = Some(tidptr);
        self.getpid()
    }

    pub fn fadvise64(
        &mut self,
        fd: u64,
        offset: u64,
        len: u64,
        advice: u64,
    ) -> SyscallResult {
        // TODO: Maybe do some implementation.
        //       For now this suffices, since
        //       it is only non-binding advice
        //       for the kernel
        Ok(0)
    }

    pub fn set_robust_list(
        &mut self,
        robust_list_head: EVAddr,
        size: u64,
    ) -> SyscallResult {
        /* todo better size check */
        if size != 24 {
            trace!("size = {}", size);
            Err(LinuxKernelError::EINVAL)?
        }
        /* todo NULL check */
        self.robust_list_head = Some(robust_list_head);
        Ok(0)
    }

    pub fn signalfd4(
        &mut self,
        fd: u64,
        mask: EVAddr,
        sizemask: u64,
        _flags: u64,
    ) -> SyscallResult {
        if fd < 1 << 31 {
            //TODO besser auf -1 prüfen
            todo!("fd != -1");
        }
        if mask == EVA_NULL {
            return Err(LinuxKernelError::EINVAL.into());
        }

        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
        if sizemask != 8 {
            //todo architure specific
            Err(LinuxKernelError::EINVAL)?
        }

        let sigmask = memory
            .read_bytes(mask, sizemask as usize)
            .map(|s| Endianness::read_u64(&s))?;
        let fd = sys_component_mut!(self, fd_map)
            .insert(&self.task_id, FileDescriptor::new(Box::new(SignalFDOps::new())))?;
        self.signalhandler.insert_fd(fd);
        Ok(fd.into())
    }

    pub fn prlimit64(
        &mut self,
        pid: u64,
        resource: u64,
        new_rlim: EVAddr,
        old_rlim: EVAddr,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let new_rlim: Option<Rlimit> = if new_rlim != EVA_NULL {
            Some(
                sys_component_mut!(self, memory)
                    .accessor(&mut cpu, self.task_id)
                    .read(new_rlim)?,
            )
        } else {
            None
        };
        let rlimit = match RLIMIT::from(resource) {
            RLIMIT::CPU => todo!(),
            RLIMIT::FSIZE => todo!(),
            RLIMIT::DATA => todo!(),
            RLIMIT::STACK => {
                if new_rlim.is_some() {
                    todo!("Need to implement setting stack size")
                }
                Rlimit {
                    rlim_cur: sys_component!(self, memory).stack_size(self.task_id)? as u64,
                    rlim_max: sys_component!(self, memory).stack_size(self.task_id)? as u64,
                }
            }
            RLIMIT::CORE => todo!(),
            RLIMIT::RSS => todo!(),
            RLIMIT::NPROC => todo!(),
            RLIMIT::NOFILE => todo!(),
            RLIMIT::MEMLOCK => todo!(),
            RLIMIT::AS => todo!(),
            RLIMIT::LOCKS => todo!(),
            RLIMIT::SIGPENDING => todo!(),
            RLIMIT::MSGQUEUE => todo!(),
            RLIMIT::NICE => todo!(),
            RLIMIT::RTPRIO => todo!(),
            RLIMIT::RTTIME => todo!(),
            RLIMIT::INVALID => todo!(),
        };
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write(old_rlim, &rlimit)?;
        Ok(0)
    }

    pub fn rt_sigaction(
        &mut self,
        signum: u64,
        act_ptr: EVAddr,
        old_ptr: u64,
        sigactionsize: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let memory = memory.accessor(&mut cpu, self.task_id);
        // TODO: directly read the decoded sigaction. Use associated type for Arch and then decode this type with the
        //       generic read functions in the memory accessor
        let action = memory.read_bytes(act_ptr, 32)?; //todo make arch dependent
        let action = Arch::read_sigaction(&action);
        self.signalhandler.insert(signum as u8, action);
        Ok(0)
    }

    pub fn rt_sigprocmask(
        &mut self,
        how: u64,
        new_set: EVAddr,
        old_set: EVAddr,
        sigsetsize: u64,
    ) -> SyscallResult {
        let mut cpu = sys_component_mut!(self, cpu);
        let mut memory = sys_component_mut!(self, memory);
        let mut memory = memory.accessor(&mut cpu, self.task_id);
        if sigsetsize != 8 {
            //todo architure specific
            Err(LinuxKernelError::EINVAL)?
        }
        let oset = self.signalhandler.getmask();
        if new_set != EVA_NULL {
            let set = memory
                .read_bytes(new_set, sigsetsize as usize)
                .map(|s| Endianness::read_u64(&s))?;
            let how = SignalMaskFlags::try_from(how)?;
            self.signalhandler.setmask(how, set);
        }
        if old_set != EVA_NULL {
            // TODO: Clean up and adjust to new memory interface. The memory interface in general needs some reworking still
            let mut old = memory.read_bytes(old_set, sigsetsize as usize)?;
            Endianness::write_u64(&mut old, oset);
            memory.write_direct(old_set, old.as_slice())?;
        }
        Ok(0)
    }

    pub fn ioctl(
        &mut self,
        fd: u64,
        request: u64,
        data: EVAddr,
    ) -> SyscallResult {
        // TODO: Implement correctly
        Err(LinuxKernelError::ENOSYS.into())
    }

    pub fn getrandom(
        &mut self,
        buf: EVAddr,
        buflen: u64,
        flags: u64,
    ) -> SyscallResult {
        // TODO: Get random data from !deterministic! source controlled by user (the fuzzer)
        //       Random sources are also a form of input to the SUT
        let data = sys_component_mut!(self, prng_map)
            .get_random_bytes(self.task_id, buflen as usize);
        let mut cpu = sys_component_mut!(self, cpu);
        sys_component_mut!(self, memory)
            .accessor(&mut cpu, self.task_id)
            .write_direct(buf, &data)?;
        Ok(buflen)
    }

    pub fn rseq(
        &mut self,
        rseq: EVAddr,
        rseq_len: u64,
        flags: u64,
        sig: u64,
    ) -> SyscallResult {
        // TODO: Implement rseq https://git.kernel.org/pub/scm/libs/librseq/librseq.git/tree/doc/man/rseq.2
        Err(LinuxKernelError::ENOSYS.into())
    }
}

#[derive(Encode, Decode, RequiredMem, Debug)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[derive(Encode, Decode, RequiredMem, Debug)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

// TODO: Make types architecture dependant
#[derive(Encode, Decode, RequiredMem)]
#[repr(C)]
struct Rlimit {
    rlim_cur: u64,
    rlim_max: u64,
}

#[repr(u64)]
#[derive(FromPrimitive)]
#[allow(clippy::style)]
enum RLIMIT {
    CPU        = 0,
    FSIZE      = 1,
    DATA       = 2,
    STACK      = 3,
    CORE       = 4,
    RSS        = 5,
    NPROC      = 6,
    NOFILE     = 7,
    MEMLOCK    = 8,
    AS         = 9,
    LOCKS      = 10,
    SIGPENDING = 11,
    MSGQUEUE   = 12,
    NICE       = 13,
    RTPRIO     = 14,
    RTTIME     = 15,
    #[default]
    INVALID,
}

impl<Endianness, Arch, CpuT, TcpT> Restorable for LinuxKernelTask<Endianness, Arch, CpuT, TcpT>
where
    Endianness: ByteOrder,
    Arch: Architecture,
    CpuT: CPU + MMU,
    TcpT: TCPSystem,
{
    fn take_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}

fn getperms(ph: &program_header::ProgramHeader) -> Permission {
    let mut perms = Permission::NONE;
    if ph.is_read() {
        perms |= Permission::READ;
    }
    if ph.is_write() {
        perms |= Permission::WRITE;
    }
    if ph.is_executable() {
        perms |= Permission::EXEC;
    }
    perms
}
