use std::cell::RefCell;
use std::collections::hash_map::Keys;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use goblin::elf::Elf;
use linux_libc_auxv::AuxVarType;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::address::{EVAddr, EVA_NULL};
use crate::emulator::abstractions::cpu::{RegisterX86, CPU};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::mmu::{MemoryError, Permission, MMU};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::filedescriptors::FileHandle;
pub use crate::emulator::platform::linux::kernel::error::LinuxKernelError;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::signals::{SignalHandler, SignalMaskFlags};
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryRead, MemoryWrite};
use crate::emulator::system::{default_handles, System};

mod error;
pub mod eventpoll;
pub mod fs;
pub mod memory;
pub mod socket;
pub mod task_id;

struct HandleMap<Endianness: ByteOrder> {
    handles: HashMap<u64, Box<dyn Handle<Endianness>>>,
    free_fds: Vec<u64>,
    replacement_handles: HashMap<u64, VecDeque<Box<dyn Handle<Endianness>>>>,
}

impl<Endianness: ByteOrder> HandleMap<Endianness> {
    fn new() -> Self {
        Self {
            handles: HashMap::new(),
            free_fds: Vec::new(),
            replacement_handles: HashMap::new(),
        }
    }
}

impl<Endianness: ByteOrder> HandleMap<Endianness> {
    /// This function is used to add a replacement handle that is inserted instead of the actual handle
    /// for the first call that is associated with the supplied fd. If multiple replacements are added
    /// for the same fd, they will be used subsequently for each new insert that happens after the fd
    /// was closed, i.e. after it was removed
    /// # Arguments
    ///
    /// * `fd`: The fd the replacement is associated with.
    /// * `handle`: The replacement handle.
    ///
    /// returns: ()
    fn add_replacement_handle(
        &mut self,
        fd: u64,
        handle: Box<dyn Handle<Endianness>>,
    ) {
        // TODO: We need to be able to skip handles and only replace later ones
        let handles = self.replacement_handles.entry(fd).or_default();
        handles.push_front(handle);
    }

    // TODO: Need to implement dup. We will most likely have to move to RC or a different datastructure that can store
    //       references to its own members
    // TODO: Need to associate pid with fd as well. If we implement fork() we will need to be able to have the same fd
    //       for multiple processes. Only the close operation will then delete the fd for one of the processes.
    //       Maybe like this?
    //       (fd, pid) -> internal_fd_id
    //       internal_fd_id -> Box<dyn Handle>
    fn insert(
        &mut self,
        fd: u64,
        handle: Box<dyn Handle<Endianness>>,
    ) -> Option<Box<dyn Handle<Endianness>>> {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        self.handles.insert(
            fd,
            self.replacement_handles
                .entry(fd)
                .or_default()
                .pop_back()
                .unwrap_or(handle),
        )
    }

    fn add(
        &mut self,
        handle: Box<dyn Handle<Endianness>>,
    ) -> u64 {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        let fd = self.get_free_fd();
        let old = self.insert(fd, handle);
        if old.is_some() {
            panic!("Add must only insert into unused keys");
        }
        fd
    }

    fn get_mut(
        &mut self,
        fd: &u64,
    ) -> Option<&mut Box<dyn Handle<Endianness>>> {
        self.handles.get_mut(fd)
    }

    fn keys(&self) -> Keys<u64, Box<dyn Handle<Endianness>>> {
        self.handles.keys()
    }

    fn get(
        &self,
        fd: &u64,
    ) -> Option<&Box<dyn Handle<Endianness>>> {
        self.handles.get(fd)
    }

    fn remove(
        &mut self,
        fd: &u64,
    ) -> Option<Box<dyn Handle<Endianness>>> {
        let old = self.handles.remove(fd);
        if old.is_some() {
            self.free_fds.push(*fd);
        }
        old
    }

    fn get_free_fd(&mut self) -> u64 {
        // TODO: Implement max fd handling (e.g. 1024?)
        self.free_fds
            .pop()
            .unwrap_or_else(|| *self.handles.keys().max().unwrap_or(&2) + 1)
    }
}

/* eventuell nach task umbenennen */
// TODO: Move thread shared data to thread specific struct that is shared between kernel task objects (rcrefcell?)
pub struct LinuxKernelTask<Endianness: ByteOrder, Arch: Architecture> {
    system: Rc<System<Endianness>>,
    handles: HandleMap<Endianness>,
    clear_child_tid: Option<EVAddr>,
    robust_list_head: Option<EVAddr>,
    signalhandler: SignalHandler<Arch::SignalFlags>,
    task_id: TaskId,
    exited: bool,
    _arch: PhantomData<Arch>,
}

type SyscallResult = Result<u64, LinuxKernelError>;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn new(
        system: Rc<System<Endianness>>,
        pid: u32,
        tid: u32,
    ) -> Self {
        LinuxKernelTask {
            system,
            handles: HandleMap::new(),
            clear_child_tid: None,
            robust_list_head: None,
            signalhandler: SignalHandler::new(),
            task_id: TaskId::new(pid, tid),
            exited: false,
            _arch: PhantomData,
        }
    }

    pub fn load_binary(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), LinuxKernelError> {
        let (interpreter_path, entrypoint) = self.load_program_sections(cpu, binary_path)?;
        self.initstack(cpu, binary_path, binary_args, binary_env, interpreter_path, entrypoint)?;
        cpu.reg_write(RegisterX86::RIP.into(), entrypoint.into())?;
        Ok(())
    }

    fn load_program_sections(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
    ) -> Result<(PathBuf, EVAddr), LinuxKernelError> {
        let mut memory = self.system.memory.borrow_mut();
        let fs = self.system.filesystem.borrow();

        memory.new_process(cpu, self.task_id)?;
        let buffer = fs.read_all(fs.resolve_last(binary_path)?)?;
        let elf = goblin::elf::Elf::parse(&buffer).unwrap();
        let interpreter_path = Path::new(elf.interpreter.ok_or(LinuxKernelError::ENOENT)?).to_owned();
        let buffer = fs.read_all(fs.resolve_last(&interpreter_path)?)?;
        let interpreter = goblin::elf::Elf::parse(&buffer).unwrap();
        //TODO iterator
        for ph in interpreter.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD {
                let perms = getperms(&ph);
                if ph.p_offset + ph.p_filesz > buffer.len().try_into().unwrap() {
                    return Err(LinuxKernelError::ENOEXEC);
                }
                let data: &[u8] = &buffer[ph.file_range()];
                let vm_range = EVAddr::from(ph.p_vaddr)..(EVAddr::from(ph.p_vaddr) + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                memory
                    .map_data(cpu, self.task_id, vm_range, data, perms)
                    .unwrap();
            }
        }
        drop(fs);
        Ok((interpreter_path, interpreter.entry.into()))
    }

    fn initstack(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
        interpreter_path: PathBuf,
        entrypoint: EVAddr,
    ) -> Result<(), LinuxKernelError> {
        let mut memory = self.system.memory.borrow_mut();
        let mut argv_strings = vec![interpreter_path.to_str().unwrap(), binary_path.to_str().unwrap()];
        argv_strings.extend_from_slice(binary_args);

        let stack_base = memory.stack_base(self.task_id).expect("no process found");
        info!("Initializing stack");
        cpu.reg_write(
            RegisterX86::RSP.into(),
            stack_base.into(),
        )
        .expect("failed to write RSP");
        let mut aux: Vec<(u64, u64)> = Vec::new();
        crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, 0u64, RegisterX86::RSP.into())?; // Stack end marker

        // Push env strings. Pointers are saved in a null terminated env array.
        let env = {
            let mut env = Vec::<u64>::new();
            for &env_str in binary_env {
                env.push(crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, env_str, RegisterX86::RSP.into())?);
            }
            env.push(0); // End of array nullptr
            env
        };

        // Push argv strings. Pointers are saved in a null terminated argv array, prefixed with the length (argc).
        let argv = {
            let mut argv = Vec::<u64>::new();
            argv.push(argv_strings.len() as u64);
            for arg in argv_strings {
                argv.push(crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, arg, RegisterX86::RSP.into())?);
            }
            argv.push(0); // End of array nullptr
            argv
        };

        aux.push((AuxVarType::Pagesz as u64, 4096));
        aux.push((AuxVarType::HwCap as u64, 0xbfebfbff));
        aux.push((AuxVarType::Uid as u64, 1));
        aux.push((AuxVarType::Gid as u64, 1));
        aux.push((AuxVarType::EUid as u64, 1));
        aux.push((AuxVarType::EGid as u64, 1));
        aux.push((AuxVarType::Platform as u64, crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, "x86_64", RegisterX86::RSP.into())?));
        aux.push((AuxVarType::Entry as u64, entrypoint.into()));
        aux.push((AuxVarType::Phdr as u64, 56));
        aux.push((AuxVarType::Phent as u64, 11));
        aux.push((AuxVarType::Secure as u64, 0));
        aux.push((0, 0));

        // align stack
        let rsp = cpu
            .reg_read(RegisterX86::RSP.into())
            .expect("error can't read RSP");
        if (rsp & !0xf) != rsp {
            cpu.reg_write(RegisterX86::RSP.into(), rsp & !0xf).unwrap();
        }

        crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, &aux, RegisterX86::RSP.into())?;
        crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, &env, RegisterX86::RSP.into())?;
        crate::push_stack!(memory.accessor(cpu, self.task_id), cpu, &argv, RegisterX86::RSP.into())?;

        let rsp = EVAddr::from(cpu.reg_read(RegisterX86::RSP.into())?);
        println!("Stack dump:\n");

        hexdump::hexdump(memory.accessor(cpu, self.task_id).read_bytes(rsp, stack_base - rsp)?.as_slice());
        debug!("Stack pointer after setup: {:#x?}", cpu.reg_read(RegisterX86::RSP.into()));

        Ok(())
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub fn add_stdio_handles(&mut self) {
        self.handles
            .insert(0, Box::new(default_handles::ReadOnly::new()));
        self.handles
            .insert(1, Box::new(default_handles::WriteOnly::new()));
        self.handles
            .insert(2, Box::new(default_handles::WriteOnly::new()));
    }

    pub fn brk(
        &mut self,
        mmu: &mut impl MMU,
        addr: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        if addr != EVA_NULL {
            memory.realloc_heap(self.task_id, addr)?;
        }
        Ok(memory.heap_end(self.task_id)?.into())
    }

    pub fn arch_prctl(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        code: u64,
        addr: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(cpu, self.task_id);
        match code {
            0x1001 => {
                //ARCH_SET_GS
                Err(LinuxKernelError::EINVAL)
            }
            0x1002 => {
                //ARCH_SET_FS
                let fsbytes = memory.read_bytes(addr, 8).unwrap();
                let fs = Endianness::read_u64(&fsbytes);
                cpu.reg_write(RegisterX86::FS_BASE.into(), fs).unwrap();
                Ok(0)
            }
            0x1003 => Err(LinuxKernelError::EINVAL), //ARCH_GET_FS
            0x1004 => Err(LinuxKernelError::EINVAL), //ARCH_GET_GS
            _ => Err(LinuxKernelError::EINVAL),
        }
    }

    pub fn gettid(
        &mut self,
        _: &mut impl CPU,
    ) -> SyscallResult {
        // TODO: Handle tid separately once threads are implemented
        Ok(self.task_id.tid.into())
    }

    pub fn time(
        &mut self,
        mmu: &mut impl MMU,
        tloc: EVAddr,
    ) -> SyscallResult {
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write_direct(tloc, &(self.system.clock.borrow().time().as_secs() as i64))?;
        Ok(0)
    }

    pub fn clock_gettime(
        &mut self,
        mmu: &mut impl MMU,
        clk_id: u64,
        tp: EVAddr,
    ) -> SyscallResult {
        let time = self.system.clock.borrow().time();
        let ts = Timespec::<Endianness> {
            tv_sec: time.as_secs() as i64,
            tv_nsec: time.subsec_nanos() as i64,
            _phantom: Default::default(),
        };
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write(tp, &ts)
            .unwrap();
        Ok(0)
    }

    pub fn getpid(
        &mut self,
        _: &mut impl CPU,
    ) -> SyscallResult {
        // TODO: Implement multi threading / processing
        Ok(self.task_id.pid.into())
    }

    pub fn execve(
        &mut self,
        _: &mut impl CPU,
        filename: EVAddr,
        argv: EVAddr,
        envp: EVAddr,
    ) -> SyscallResult {
        // TODO: Stop emulation and replace running process with different process.
        Ok(0)
    }

    pub fn uname(
        &mut self,
        mmu: &mut impl MMU,
        buf: EVAddr,
    ) -> SyscallResult {
        //TODO check if address is mapped and has space for struct new_utsname
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
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
        _: &mut impl CPU,
        fd: u64,
        cmd: u64,
        arg: u64,
    ) -> SyscallResult {
        // TODO: Implement correctly
        warn!("FCNTL not implemented fully yet");
        self.handles.get_mut(&fd);
        Ok(2)
    }

    pub fn gettimeofday(
        &mut self,
        mmu: &mut impl MMU,
        tv: EVAddr,
        tz: EVAddr,
    ) -> SyscallResult {
        if tv == EVA_NULL {
            Err(LinuxKernelError::EINVAL)?
        }
        if tz != EVA_NULL {
            todo!("Timezone not implemented yet")
        }
        let time = self.system.clock.borrow().time();
        let timeval = Timeval::<Endianness> {
            tv_sec: time.as_secs() as i64,
            tv_usec: time.subsec_micros() as i64,
            _phantom: Default::default(),
        };
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write(tv, &timeval)?;
        Ok(0)
    }

    pub fn access(
        &mut self,
        _: &mut impl CPU,
        _filename: u64,
        _mode: u64,
    ) -> SyscallResult {
        Err(LinuxKernelError::EINVAL)
    }

    pub fn set_tid_address(
        &mut self,
        uc: &mut impl CPU,
        tidptr: EVAddr,
    ) -> SyscallResult {
        /* todo NULL check */
        self.clear_child_tid = Some(tidptr);
        self.getpid(uc)
    }

    pub fn fadvise64(
        &mut self,
        uc: &mut impl CPU,
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
        _: &mut impl CPU,
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
        _: &mut impl CPU,
        fd: u64,
        mask: EVAddr,
        flags: u64,
        sizemask: u64,
    ) -> SyscallResult {
        todo!()
    }

    pub fn prlimit64(
        &mut self,
        mmu: &mut impl MMU,
        pid: u64,
        resource: u64,
        new_rlim: EVAddr,
        old_rlim: EVAddr,
    ) -> SyscallResult {
        let new_rlim: Option<Rlimit> = if new_rlim != EVA_NULL {
            Some(
                self.system
                    .memory
                    .borrow_mut()
                    .accessor(mmu, self.task_id)
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
                    rlim_cur: self.system.memory.borrow().stack_size(self.task_id)? as u64,
                    rlim_max: self.system.memory.borrow().stack_size(self.task_id)? as u64,
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
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write(old_rlim, &rlimit)?;
        Ok(0)
    }

    pub fn rt_sigaction(
        &mut self,
        mmu: &mut impl MMU,
        signum: u64,
        act_ptr: EVAddr,
        old_ptr: u64,
        sigactionsize: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let memory = memory.accessor(mmu, self.task_id);
        // TODO: directly read the decoded sigaction. Use associated type for Arch and then decode this type with the
        //       generic read functions in the memory accessor
        let action = memory.read_bytes(act_ptr, 32)?; //todo make arch dependent
        let action = Arch::read_sigaction(&action);
        self.signalhandler.insert(signum as u8, action);
        Ok(0)
    }

    pub fn rt_sigprocmask(
        &mut self,
        mmu: &mut impl MMU,
        how: u64,
        new_set: EVAddr,
        old_set: EVAddr,
        sigsetsize: u64,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
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
        _: &mut impl CPU,
        fd: u64,
        request: u64,
        data: EVAddr,
    ) -> SyscallResult {
        // TODO: Implement correctly
        Err(LinuxKernelError::ENOSYS)
    }

    pub fn getrandom(
        &mut self,
        mmu: &mut impl MMU,
        buf: EVAddr,
        buflen: u64,
        flags: u64,
    ) -> SyscallResult {
        // TODO: Get random data from !deterministic! source controlled by user (the fuzzer)
        //       Random sources are also a form of input to the SUT
        let data = vec![0u8; buflen as usize];
        self.system
            .memory
            .borrow_mut()
            .accessor(mmu, self.task_id)
            .write_direct(buf, &data)?;
        Ok(buflen)
    }

    pub fn rseq(
        &mut self,
        _: &mut impl CPU,
        rseq: EVAddr,
        rseq_len: u64,
        flags: u64,
        sig: u64,
    ) -> SyscallResult {
        // TODO: Implement rseq https://git.kernel.org/pub/scm/libs/librseq/librseq.git/tree/doc/man/rseq.2
        Err(LinuxKernelError::ENOSYS)
    }
}

pub struct Timeval<Endianness> {
    pub tv_sec: i64,
    pub tv_usec: i64,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> RequiredMem for Timeval<Endianness> {}

impl<Endianness: ByteOrder> Encode<Endianness> for Timeval<Endianness> {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; Self::required_mem()];
        Endianness::write_i64(&mut d[..8], self.tv_sec);
        Endianness::write_i64(&mut d[8..], self.tv_usec);
        d
    }
}

impl<Endianness: ByteOrder> TryFrom<&[u8]> for Timeval<Endianness> {
    type Error = GenericError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != mem::size_of::<i64>() * 2 {
            Err(GenericError::new(GenericErrorKind::Unknown, "Invalid length"))
        } else {
            Ok(Timeval {
                tv_sec: Endianness::read_i64(&value[..8]),
                tv_usec: Endianness::read_i64(&value[8..]),
                _phantom: Default::default(),
            })
        }
    }
}

pub struct Timespec<Endianness: ByteOrder> {
    pub tv_sec: i64,
    pub tv_nsec: i64,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> RequiredMem for Timespec<Endianness> {}

impl<Endianness: ByteOrder> Encode<Endianness> for Timespec<Endianness> {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; mem::size_of::<Self>()];
        Endianness::write_i64(&mut d[..8], self.tv_sec);
        Endianness::write_i64(&mut d[8..], self.tv_nsec);
        d
    }
}

impl<Endianness: ByteOrder> TryFrom<&[u8]> for Timespec<Endianness> {
    type Error = GenericError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != mem::size_of::<i64>() * 2 {
            Err(GenericError::new(GenericErrorKind::Unknown, "Invalid length"))
        } else {
            Ok(Timespec {
                tv_sec: Endianness::read_i64(&value[..8]),
                tv_nsec: Endianness::read_i64(&value[8..]),
                _phantom: Default::default(),
            })
        }
    }
}

// TODO: Make types architecture dependant
#[repr(C)]
struct Rlimit {
    rlim_cur: u64,
    rlim_max: u64,
}

impl<Endianness: ByteOrder> Encode<Endianness> for Rlimit {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; mem::size_of::<Self>()];
        Endianness::write_u64(&mut d[..8], self.rlim_cur);
        Endianness::write_u64(&mut d[8..], self.rlim_max);
        d
    }
}

// TODO: Write derive macro or something like this to automate the from trait?
impl<Endianness: ByteOrder> Decode<Endianness> for Rlimit {
    fn decode(value: &[u8]) -> Result<Self, MemoryError> {
        if value.len() != mem::size_of::<Rlimit>() {
            Err(MemoryError::InvalidSize)
        } else {
            Ok(Rlimit {
                rlim_cur: Endianness::read_u64(&value[..8]),
                rlim_max: Endianness::read_u64(&value[8..]),
            })
        }
    }
}

impl RequiredMem for Rlimit {}

#[repr(u64)]
#[derive(FromPrimitive)]
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

impl<Endianness: ByteOrder, Arch: Architecture> Restorable for LinuxKernelTask<Endianness, Arch> {
    fn snapshot(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self) -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self) -> Result<(), GenericError> {
        todo!()
    }
}

fn getperms(ph: &goblin::elf::program_header::ProgramHeader) -> Permission {
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
