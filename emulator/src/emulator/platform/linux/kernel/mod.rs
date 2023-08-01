use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::ByteOrder;
use goblin::elf::{Elf, program_header};
use num_enum::FromPrimitive;

use error::SyscallError;
use fuzzyemu_derive::{Decode, Encode, RequiredMem};

use crate::emulator::abstractions::cpu::{CPU, RegisterX86};
use crate::emulator::abstractions::cpu::address::{EVA_NULL, EVAddr};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::file_descriptors::FileDescriptorOps;
use crate::emulator::abstractions::mmu::{MMU, Permission};
use crate::emulator::abstractions::mmu::encoding::{Decode, Encode, RequiredMem};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::macros::push_stack;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::fd::{FD, FileDescriptorData};
use crate::emulator::platform::linux::kernel::auxv::AUXVType;
pub use crate::emulator::platform::linux::kernel::error::LinuxKernelError;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::LinuxSystem;
use crate::emulator::platform::linux::signalfd::SignalFDOps;
use crate::emulator::platform::linux::signals::{SignalHandler, SignalMaskFlags};
use crate::emulator::system::default_handles;
use crate::emulator::system::fd_map::FileDescriptor;
use crate::emulator::system::memory::rw_interface::{DirectMemoryWrite, MemoryRead, MemoryWrite};

pub(crate) mod auxv;
pub(crate) mod error;
pub(crate) mod eventpoll;
pub(crate) mod fs;
pub(crate) mod memory;
pub(crate) mod net;
pub(crate) mod socket;
pub(crate) mod task_id;

/* eventuell nach task umbenennen */
// TODO: Move thread shared data to thread specific struct that is shared between kernel task objects (rcrefcell?)
pub struct LinuxKernelTask<Endianness: ByteOrder, Arch: Architecture> {
    system: Rc<LinuxSystem<Endianness>>,
    clear_child_tid: Option<EVAddr>,
    robust_list_head: Option<EVAddr>,
    signalhandler: SignalHandler<Arch::SignalFlags>,
    task_id: TaskId,
    entrypoint: Option<EVAddr>,
    _arch: PhantomData<Arch>,
}

type SyscallResult = Result<u64, SyscallError>;

impl<Endianness: ByteOrder + 'static, Arch: Architecture> LinuxKernelTask<Endianness, Arch> {
    pub fn new(
        system: Rc<LinuxSystem<Endianness>>,
        pid: u32,
        tid: u32,
    ) -> Self {
        LinuxKernelTask {
            system,
            clear_child_tid: None,
            robust_list_head: None,
            signalhandler: SignalHandler::new(),
            task_id: TaskId::new(pid, tid),
            entrypoint: None,
            _arch: PhantomData,
        }
    }

    pub fn entrypoint(&self) -> Option<EVAddr> {
        self.entrypoint
    }

    pub fn load_binary(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), LinuxKernelError> {
        let (interpreter_path, entrypoint, phdr) = self.load_program_sections(cpu, binary_path)?;
        self.initstack(cpu, binary_path, binary_args, binary_env, interpreter_path, entrypoint, phdr)?;
        cpu.reg_write(RegisterX86::RIP.into(), entrypoint.into())?;
        Ok(())
    }

    fn load_program_sections(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
    ) -> Result<(PathBuf, EVAddr, u64), LinuxKernelError> {
        self.system.memory.borrow_mut().new_process(cpu, self.task_id)?;
        let fs = self.system.filesystem.borrow();

        let program_data = fs.read_all(fs.resolve_last(binary_path)?)?;
        let program = goblin::elf::Elf::parse(&program_data).unwrap();
        let program_base = EVAddr::from(0x0000_c000_0000_0000);
        self.entrypoint = Some(EVAddr::from(program.entry));

        // Load interpreter
        let interpreter_path = Path::new(program.interpreter.ok_or(LinuxKernelError::ENOENT)?).to_owned();
        let interpreter_data = fs.read_all(fs.resolve_last(&interpreter_path)?)?;
        let interpreter = goblin::elf::Elf::parse(&interpreter_data).unwrap();
        let interpreter_base = EVAddr::from(0x0000_b000_0000_0000);
        //TODO iterator
        drop(fs);
        let phdr = program_base + program.program_headers.iter().find(|ph| ph.p_type == program_header::PT_PHDR).unwrap().p_offset;
        self.system.memory.borrow_mut().map_data(cpu, self.task_id, program_base..program_base+program_data.len(), program_data.as_slice(), Permission::READ | Permission::WRITE);

        self.load_interpreter(cpu, interpreter_base, &interpreter, &interpreter_data)?;
        Ok((interpreter_path, interpreter_base + EVAddr::from(interpreter.entry), phdr.into()))
    }

    fn load_interpreter(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        base: EVAddr,
        elf: &Elf,
        elf_data: &Vec<u8>,
    ) -> Result<(), LinuxKernelError> {
        let mut memory = self.system.memory.borrow_mut();
        for ph in &elf.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD {
                let perms = getperms(&ph);
                if ph.p_offset + ph.p_filesz > elf_data.len().try_into().unwrap() {
                    return Err(LinuxKernelError::ENOEXEC);
                }
                let data: &[u8] = &elf_data[ph.file_range()];
                let vm_range = base + EVAddr::from(ph.p_vaddr)..(base + EVAddr::from(ph.p_vaddr) + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                memory
                    .map_data(cpu, self.task_id, vm_range, data, perms)
                    .unwrap();
            }
        }
        Ok(())
    }

    fn initstack(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
        binary_args: &[&str],
        binary_env: &[&str],
        interpreter_path: PathBuf,
        entrypoint: EVAddr,
        phdr: u64,
    ) -> Result<(), LinuxKernelError> {
        let mut memory = self.system.memory.borrow_mut();
        let mut argv_strings = vec![binary_path.to_str().unwrap()];
        argv_strings.extend_from_slice(binary_args);

        let stack_base = memory.stack_base(self.task_id).expect("no process found");
        info!("Initializing stack");
        cpu.reg_write(RegisterX86::RSP.into(), stack_base.into())
            .expect("failed to write RSP");
        let mut aux: Vec<(u64, u64)> = Vec::new();
        push_stack!(memory.accessor(cpu, self.task_id), cpu, 0u64, RegisterX86::RSP.into())?; // Stack end marker

        // Push env strings. Pointers are saved in a null terminated env array.
        let env = {
            let mut env = Vec::<u64>::new();
            for &env_str in binary_env {
                env.push(push_stack!(memory.accessor(cpu, self.task_id), cpu, env_str, RegisterX86::RSP.into())?);
            }
            env.push(0); // End of array nullptr
            env
        };

        // Push argv strings. Pointers are saved in a null terminated argv array, prefixed with the length (argc).
        let argv = {
            let mut argv = Vec::<u64>::new();
            argv.push(argv_strings.len() as u64);
            for arg in argv_strings {
                argv.push(push_stack!(memory.accessor(cpu, self.task_id), cpu, arg, RegisterX86::RSP.into())?);
            }
            argv.push(0); // End of array nullptr
            argv
        };

        aux.push((AUXVType::Pagesz as u64, 4096));
        aux.push((AUXVType::HwCap as u64, 0xbfebfbff));
        aux.push((AUXVType::Uid as u64, 1000));
        aux.push((AUXVType::Gid as u64, 1000));
        aux.push((AUXVType::EUid as u64, 1000));
        aux.push((AUXVType::EGid as u64, 1000));
        aux.push((
            AUXVType::Platform as u64,
            push_stack!(memory.accessor(cpu, self.task_id), cpu, "x86_64", RegisterX86::RSP.into())?,
        ));
        aux.push((AUXVType::Entry as u64, self.entrypoint.unwrap().into()));
        aux.push((AUXVType::Phdr as u64, phdr));
        aux.push((AUXVType::Phent as u64, 56));
        aux.push((AUXVType::Phnum as u64, 12));
        aux.push((AUXVType::Base as u64, 0x0000_b000_0000_0000));
        aux.push((AUXVType::Flags as u64, 0));
        aux.push((AUXVType::Secure as u64, 0));
        aux.push((0, 0));

        // align stack
        let rsp = cpu
            .reg_read(RegisterX86::RSP.into())
            .expect("error can't read RSP");
        if (rsp & !0xf) != rsp {
            cpu.reg_write(RegisterX86::RSP.into(), rsp & !0xf).unwrap();
        }

        push_stack!(memory.accessor(cpu, self.task_id), cpu, &aux, RegisterX86::RSP.into())?;
        push_stack!(memory.accessor(cpu, self.task_id), cpu, &env, RegisterX86::RSP.into())?;
        push_stack!(memory.accessor(cpu, self.task_id), cpu, &argv, RegisterX86::RSP.into())?;

        let rsp = EVAddr::from(cpu.reg_read(RegisterX86::RSP.into())?);

        Ok(())
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub fn add_stdio_handles(&mut self) {
        let mut file_descriptor_map = self.system.fd_map.borrow_mut();
        file_descriptor_map.insert_at(
            &self.task_id,
            0.into(),
            FileDescriptor::new(Box::new(default_handles::ReadOnly::new())),
        );
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
        mmu: &mut impl MMU,
        addr: EVAddr,
    ) -> SyscallResult {
        let mut memory = self.system.memory.borrow_mut();
        if addr != EVA_NULL {
            memory
                .realloc_heap(self.task_id, addr)
                .inspect_err(|e| warn!("Error during brk: {e:?}"));
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

    pub fn futex(
        &mut self,
        mmu: &mut impl MMU,
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
        mmu: &mut impl MMU,
        clk_id: u64,
        tp: EVAddr,
    ) -> SyscallResult {
        let time = self.system.clock.borrow().time();
        let ts = Timespec {
            tv_sec: time.as_secs() as i64,
            tv_nsec: time.subsec_nanos() as i64,
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
        fd: FD,
        cmd: u64,
        arg: u64,
    ) -> SyscallResult {
        // TODO: Implement correctly
        error!("FCNTL not implemented fully yet");
        self.system.fd_map.borrow_mut().get(&self.task_id, &fd);
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
        let timeval = Timeval {
            tv_sec: time.as_secs() as i64,
            tv_usec: time.subsec_micros() as i64,
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
        Err(LinuxKernelError::EINVAL.into())
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
        mmu: &mut impl MMU,
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

        let mut memory = self.system.memory.borrow_mut();
        let mut memory = memory.accessor(mmu, self.task_id);
        if sizemask != 8 {
            //todo architure specific
            Err(LinuxKernelError::EINVAL)?
        }

        let sigmask = memory
            .read_bytes(mask, sizemask as usize)
            .map(|s| Endianness::read_u64(&s))?;
        let fd = self
            .system
            .fd_map
            .borrow_mut()
            .insert(&self.task_id, FileDescriptor::new(Box::new(SignalFDOps::new())))?;
        self.signalhandler.insert_fd(fd);
        Ok(fd.into())
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
        Err(LinuxKernelError::ENOSYS.into())
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
        Err(LinuxKernelError::ENOSYS.into())
    }
}

#[derive(Encode, Decode, RequiredMem)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[derive(Encode, Decode, RequiredMem)]
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
