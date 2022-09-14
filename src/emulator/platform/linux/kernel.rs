use std::cell::{RefCell, RefMut};
use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Keys;
use std::marker::PhantomData;
use std::mem;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use linux_libc_auxv::AuxVarType;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::{CPU, CPUError, Hooks, MMU, Permission, RegisterX86, Vaddr};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::handles::Handle;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::Architecture;
use crate::emulator::platform::linux::filedescriptors::FileHandle;
use crate::emulator::platform::linux::signals::{SignalHandler, SignalMaskFlags};
use crate::emulator::system::default_handles;
use crate::emulator::system::filesystem::{Filesystem, FSError};
use crate::emulator::system::memory::{Encode, Memory, MemoryError, MemoryWrite};
use crate::read_osstring;

pub mod fs;
pub mod socket;
pub mod eventpoll;

const MAP_ANONYMOUS: u64 = 0x20;
const MAP_PRIVATE: u64 = 0x2;
const MAP_FIXED: u64 = 0x10;
const MAP_DENYWRITE: u64 = 0x0800;

struct HandleMap<Endianness: ByteOrder> {
    handles: HashMap<u64, Box<dyn Handle<Endianness>>>,
    replacement_handles: HashMap<u64, VecDeque<Box<dyn Handle<Endianness>>>>,
}

impl<Endianness: ByteOrder>
HandleMap<Endianness> {
    fn new() -> Self {
        Self {
            handles: HashMap::new(),
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
    fn add_replacement_handle(&mut self, fd: u64, handle: Box<dyn Handle<Endianness>>) {
        let handles = self.replacement_handles.entry(fd).or_default();
        handles.push_front(handle);
    }

    fn insert(&mut self, fd: u64, handle: Box<dyn Handle<Endianness>>) -> Option<Box<dyn Handle<Endianness>>> {
        // Insert the handle if no replacement exists. Otherwise insert the replacement
        self.handles.insert(fd, self.replacement_handles.entry(fd).or_default().pop_back().unwrap_or(handle))
    }

    fn get_mut(&mut self, fd: &u64) -> Option<&mut Box<dyn Handle<Endianness>>> {
        self.handles.get_mut(fd)
    }

    fn keys(&self) -> Keys<u64, Box<dyn Handle<Endianness>>> {
        self.handles.keys()
    }

    fn get(&self, fd: &u64) -> Option<&Box<dyn Handle<Endianness>>> {
        self.handles.get(fd)
    }

    fn remove(&mut self, fd: &u64) -> Option<Box<dyn Handle<Endianness>>> {
        self.handles.remove(fd)
    }
}

/* eventuell nach task umbenennen */
pub struct LinuxKernel<Endianness: ByteOrder, Arch: Architecture> {
    memory: Memory<Endianness>,
    filesystem: Rc<RefCell<Filesystem<Endianness>>>,
    handles: HandleMap<Endianness>,
    clear_child_tid: Option<Vaddr>,
    robust_list_head: Option<Vaddr>,
    signalhandler: SignalHandler<Arch::SignalFlags>,
    system_time: Duration,
    _arch: PhantomData<Arch>,
}

type SyscallResult = Result<u64, LinuxKernelError>;

impl<Endianness: ByteOrder + 'static, Arch: Architecture>
LinuxKernel<Endianness, Arch> {
    pub fn new(
        memory: Memory<Endianness>,
        filesystem: Rc<RefCell<Filesystem<Endianness>>>,
    ) -> Self {
        LinuxKernel {
            memory,
            filesystem,
            handles: HandleMap::new(),
            clear_child_tid: None,
            robust_list_head: None,
            signalhandler: SignalHandler::new(),
            system_time: Duration::from_nanos(1662647309301240322),
            _arch: PhantomData,
        }
    }

    pub fn load_binary(
        &mut self,
        cpu: &mut (impl CPU + MMU),
        binary_path: &Path,
    ) -> Result<(), LinuxKernelError> {
        let fs = self.filesystem.borrow();
        let buffer = fs.read_all(fs.resolve_last(binary_path)?)?;
        let elf = goblin::elf::Elf::parse(&buffer).unwrap();
        let interpreter_path = Path::new(elf.interpreter.ok_or(LinuxKernelError::ENOENT)?);
        let buffer = fs.read_all(fs.resolve_last(interpreter_path)?)?;
        let interpreter = goblin::elf::Elf::parse(&buffer).unwrap();
        cpu.reg_write(RegisterX86::RIP.into(), interpreter.entry).unwrap();
        //TODO iterator
        for ph in interpreter.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD {
                let perms = getperms(&ph);
                if ph.p_offset + ph.p_filesz > buffer.len().try_into().unwrap() {
                    return Err(LinuxKernelError::ENOEXEC);
                }
                let data: &[u8] = &buffer[ph.file_range()];
                let vm_range = ph.p_vaddr..(ph.p_vaddr + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                self.memory.map_data(data, vm_range, perms, &mut *cpu).unwrap();
            }
        }
        drop(fs);
        self.initstack(cpu, binary_path, interpreter_path, interpreter.entry)
    }

    fn initstack(
        &mut self,
        cpu: &mut impl CPU,
        binary_path: &Path,
        interpreter_path: &Path,
        entrypoint: Vaddr,
    ) -> Result<(), LinuxKernelError> {
        info!("Initializing stack");
        cpu.reg_write(RegisterX86::RSP.into(), 0x4000000000000000).expect("failed to write RSP");
        let mem = &mut self.memory;
        let mut env: Vec<u64> = Vec::new();
        let mut argv: Vec<u64> = Vec::new();
        let mut aux: Vec<(u64, u64)> = Vec::new();
        crate::push_stack!(mem, 0u64, RegisterX86::RSP.into(), cpu);
        env.push(crate::push_stack!(mem, "SHELL=/bin/sh", RegisterX86::RSP.into(), cpu));
        // env.push(crate::push_stack!(mem, "LD_DEBUG=all", RegisterX86::RSP.into(), cpu));
        env.push(0);
        argv.push(2);
        argv.push(crate::push_stack!(mem, interpreter_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
        argv.push(crate::push_stack!(mem, binary_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
        argv.push(0);
        aux.push((AuxVarType::Pagesz as u64, 4096));
        aux.push((AuxVarType::HwCap as u64, 0xbfebfbff));
        aux.push((AuxVarType::Uid as u64, 1));
        aux.push((AuxVarType::Gid as u64, 1));
        aux.push((AuxVarType::EUid as u64, 1));
        aux.push((AuxVarType::EGid as u64, 1));
        aux.push((AuxVarType::Platform as u64, crate::push_stack!(mem, "x86_64", RegisterX86::RSP.into(), cpu)));
        aux.push((AuxVarType::Entry as u64, entrypoint));
        aux.push((AuxVarType::Phdr as u64, 56));
        aux.push((AuxVarType::Phent as u64, 11));
        aux.push((AuxVarType::Secure as u64, 0));
        aux.push((0, 0));

        // align stack
        let rsp = cpu.reg_read(RegisterX86::RSP.into()).expect("error can't read RSP");
        if (rsp & !0xf) != rsp {
            cpu.reg_write(RegisterX86::RSP.into(), rsp & !0xf).unwrap();
        }

        crate::push_stack!(mem, &aux, RegisterX86::RSP.into(), cpu);
        crate::push_stack!(mem, &env, RegisterX86::RSP.into(), cpu);
        crate::push_stack!(mem, &argv, RegisterX86::RSP.into(), cpu);
        Ok(())
    }

    pub fn do_timestep(
        &mut self,
        proportion: u32,
    ) {
        // TODO: Pull out simulation factor and make it configurable.
        // The simulation factor determines the size of the timestep taken during each basic block.
        // let simulation_factor = 0.032f64;
        let simulation_factor = 0.32f64;
        self.system_time += Duration::from_nanos((proportion as f64 * simulation_factor) as u64);
    }

    pub fn add_stdio_handles(&mut self) {
        self.handles.insert(0, Box::new(default_handles::ReadOnly::new()));
        self.handles.insert(1, Box::new(default_handles::WriteOnly::new()));
        self.handles.insert(2, Box::new(default_handles::WriteOnly::new()));
    }

    pub fn brk(
        &mut self,
        cpu: &mut impl MMU,
        addr: u64,
    ) -> SyscallResult {
        if addr != 0 {
            self.memory.realloc_heap(addr, cpu);
        }
        Ok(self.memory.heapend())
    }

    pub fn arch_prctl(
        &mut self,
        cpu: &mut impl CPU,
        code: u64,
        addr: u64,
    ) -> SyscallResult {
        match code {
            0x1001 => { //ARCH_SET_GS
                Err(LinuxKernelError::EINVAL)
            }
            0x1002 => { //ARCH_SET_FS
                let fsbytes = self.memory.read_internal(addr, 8).unwrap();
                let fs = Endianness::read_u64(&fsbytes);
                cpu.reg_write(RegisterX86::FS_BASE.into(), fs).unwrap();
                Ok(0)
            }
            0x1003 => Err(LinuxKernelError::EINVAL), //ARCH_GET_FS
            0x1004 => Err(LinuxKernelError::EINVAL), //ARCH_GET_GS
            _ => Err(LinuxKernelError::EINVAL),
        }
    }

    pub fn time(
        &mut self,
        cpu: &mut impl MMU,
        tloc: Vaddr,
    ) -> SyscallResult {
        self.memory.write_generic(tloc, &(self.system_time.as_secs() as i64))?;
        Ok(0)
    }

    pub fn clock_gettime(
        &mut self,
        cpu: &mut impl MMU,
        clk_id: u64,
        tp: Vaddr,
    ) -> SyscallResult {
        let ts = Timespec::<Endianness> {
            tv_sec: self.system_time.as_secs() as i64,
            tv_nsec: self.system_time.subsec_nanos() as i64,
            _phantom: Default::default(),
        };
        self.memory.write_generic(tp, &ts).unwrap();
        Ok(0)
    }

    pub fn getpid(
        &mut self,
        _: &mut impl CPU,
    ) -> SyscallResult {
        // TODO: Implement multi threading / processing
        Ok(1)
    }

    fn mmap_anon(
        &mut self,
        cpu: &mut impl MMU,
        addr: Vaddr,
        length: u64,
        prot: u64,
        flags: u64,
    ) -> SyscallResult {
        let addr = if (flags & MAP_FIXED) != 0 {
            self.memory.unmap(addr, length, cpu)?;
            addr
        } else {
            //self.memory.find_free(addr, length)
            addr
        };
        let addr = match self.memory.mmap(addr, length, prot.try_into()?, cpu) {
            Ok(addr) => addr,
            Err(e) => {
                println!("mmap error {:?}", e);
                return Err(LinuxKernelError::EINVAL);
            }
        };
        Ok(addr)
    }

    fn mmap_private(
        &mut self,
        cpu: &mut impl MMU,
        addr: u64,
        length: u64,
        prot: u64,
        flags: u64,
        fd: u64,
        offset: u64,
    ) -> SyscallResult {
        if fd >= 1 << 31 {
            Err(LinuxKernelError::EINVAL)?
        }
        let addr = self.mmap_anon(cpu, addr, length, prot, flags).unwrap();
        if addr == !0 {
            Err(LinuxKernelError::EINVAL)?
        }
        let f = self.handles.get(&fd).ok_or(LinuxKernelError::EINVAL)?;
        let data = f.pread(offset, length)?;
        self.memory.write(addr, data.as_slice());
        Ok(addr)
    }

    pub fn mmap(
        &mut self,
        cpu: &mut impl MMU,
        addr: Vaddr,
        length: u64,
        prot: u64,
        flags: u64,
        fd: u64,
        offset: u64,
    ) -> SyscallResult {
        if (flags & !(MAP_ANONYMOUS | MAP_PRIVATE | MAP_FIXED | MAP_DENYWRITE)) != 0 {
            Err(LinuxKernelError::EINVAL)?
        }
        if (flags & MAP_ANONYMOUS) != 0 {
            self.mmap_anon(cpu, addr, length, prot, flags)
        } else if (flags & MAP_PRIVATE) != 0 {
            self.mmap_private(cpu, addr, length, prot, flags, fd, offset)
        } else {
            Err(LinuxKernelError::EINVAL)
        }
    }

    pub fn munmap(
        &mut self,
        cpu: &mut impl MMU,
        addr: Vaddr,
        len: u64,
    ) -> SyscallResult {
        self.memory.unmap(addr, len, cpu)?;
        Ok(0)
    }

    pub fn uname(
        &mut self,
        _: &mut impl CPU,
        buf: u64,
    ) -> SyscallResult {
        //TODO check if address is mapped and has space for struct new_utsname
        self.memory.write(buf, "Linux");
        self.memory.write(buf + 65, "ILT792");
        self.memory.write(buf + 2 * 65, "5.4.0-110-generic");
        self.memory.write(buf + 3 * 65, "#124-Ubuntu SMP Thu Apr 14 19:46:19 UTC 2022");
        self.memory.write(buf + 4 * 65, "x86_64");
        self.memory.write(buf + 5 * 65, "(none)");
        Ok(0)
    }

    pub fn gettimeofday(
        &mut self,
        _: &mut impl CPU,
        tv: Vaddr,
        tz: Vaddr,
    ) -> SyscallResult {
        if tv == 0 {
            Err(LinuxKernelError::EINVAL)?
        }
        if tz != 0 {
            todo!("Timezone not implemented yet")
        }
        let timeval = Timeval::<Endianness> {
            tv_sec: self.system_time.as_secs() as i64,
            tv_usec: self.system_time.subsec_micros() as i64,
            _phantom: Default::default(),
        };
        self.memory.write_generic(tv, &timeval)?;
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

    pub fn mprotect(
        &mut self,
        cpu: &mut impl MMU,
        start: u64,
        len: u64,
        prot: u64,
    ) -> SyscallResult {
        self.memory.protect(start, len, prot.try_into().unwrap(), cpu);
        Ok(0)
    }

    pub fn set_tid_address(
        &mut self,
        uc: &mut impl CPU,
        tidptr: u64,
    ) -> SyscallResult {
        /* todo NULL check */
        self.clear_child_tid = Some(tidptr);
        self.getpid(uc)
    }

    pub fn set_robust_list(
        &mut self,
        _: &mut impl CPU,
        robust_list_head: u64,
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

    pub fn prlimit64(
        &mut self,
        _: &mut impl CPU,
        pid: u64,
        resource: u64,
        new_rlim: Vaddr,
        old_rlim: Vaddr,
    ) -> SyscallResult {
        let new_rlim = if new_rlim != 0 { Some(self.memory.read_struct::<Rlimit<Endianness>>(new_rlim)?) } else { None };
        let rlimit = match RLIMIT::from(resource) {
            RLIMIT::CPU => todo!(),
            RLIMIT::FSIZE => todo!(),
            RLIMIT::DATA => todo!(),
            RLIMIT::STACK => {
                if new_rlim.is_some() {
                    todo!("Need to implement setting stack size")
                }
                Rlimit::<Endianness> {
                    rlim_cur: self.memory.stack_size() as u64,
                    rlim_max: self.memory.stack_size() as u64,
                    _phantom: Default::default(),
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
        self.memory.write_generic(old_rlim, &rlimit)?;
        Ok(0)
    }

    pub fn rt_sigaction(
        &mut self,
        _: &mut impl CPU,
        signum: u64,
        act_ptr: u64,
        old_ptr: u64,
        sigactionsize: u64,
    ) -> SyscallResult {
        let action = self.memory.access_mem(act_ptr, 32)?; //todo make arch dependent
        let action = Arch::read_sigaction(&action);
        self.signalhandler.insert(signum as u8, action);
        Ok(0)
    }

    pub fn rt_sigprocmask(
        &mut self,
        _: &mut impl CPU,
        how: u64,
        new_set: u64,
        old_set: u64,
        sigsetsize: u64,
    ) -> SyscallResult {
        if sigsetsize != 8 { //todo architure specific
            Err(LinuxKernelError::EINVAL)?
        }
        let oset = self.signalhandler.getmask();
        if new_set != 0 {
            let set = self.memory
                          .access_mem(new_set, sigsetsize as usize)
                          .map(|s| Endianness::read_u64(&s))?;
            let how = SignalMaskFlags::try_from(how)?;
            self.signalhandler.setmask(how, set);
        }
        if old_set != 0 {
            let mut old = self.memory.access_mem_mut(old_set, sigsetsize as usize)?;
            Endianness::write_u64(&mut old, oset);
        }
        Ok(0)
    }
}

impl<Endianness: ByteOrder> Encode<Endianness>
for i64 {
    fn encode(&self) -> Vec<u8> {
        let mut data = vec![0; mem::size_of::<Self>()];
        Endianness::write_i64(&mut data[..8], *self);
        data
    }
}

pub struct Timeval<Endianness> {
    pub tv_sec: i64,
    pub tv_usec: i64,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> Encode<Endianness>
for Timeval<Endianness> {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; mem::size_of::<Self>()];
        Endianness::write_i64(&mut d[..8], self.tv_sec);
        Endianness::write_i64(&mut d[8..], self.tv_usec);
        d
    }
}

impl<Endianness: ByteOrder> TryFrom<&[u8]>
for Timeval<Endianness> {
    type Error = GenericError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != mem::size_of::<i64>() * 2 {
            Err(GenericError::new(GenericErrorKind::Unknown, "Invalid length"))
        } else {
            Ok(Timeval {
                tv_sec: Endianness::read_i64(&value[..8]),
                tv_usec: Endianness::read_i64(&value[8..]),
                _phantom: Default::default(),
            }
            )
        }
    }
}

pub struct Timespec<Endianness: ByteOrder> {
    pub tv_sec: i64,
    pub tv_nsec: i64,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> Encode<Endianness>
for Timespec<Endianness> {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; mem::size_of::<Self>()];
        Endianness::write_i64(&mut d[..8], self.tv_sec);
        Endianness::write_i64(&mut d[8..], self.tv_nsec);
        d
    }
}

impl<Endianness: ByteOrder> TryFrom<&[u8]>
for Timespec<Endianness> {
    type Error = GenericError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != mem::size_of::<i64>() * 2 {
            Err(GenericError::new(GenericErrorKind::Unknown, "Invalid length"))
        } else {
            Ok(Timespec {
                tv_sec: Endianness::read_i64(&value[..8]),
                tv_nsec: Endianness::read_i64(&value[8..]),
                _phantom: Default::default(),
            }
            )
        }
    }
}

// TODO: Make types architecture dependant
struct Rlimit<Endianness: ByteOrder> {
    rlim_cur: u64,
    rlim_max: u64,
    _phantom: PhantomData<Endianness>,
}

impl<Endianness: ByteOrder> Encode<Endianness>
for Rlimit<Endianness> {
    fn encode(&self) -> Vec<u8> {
        let mut d = vec![0; mem::size_of::<Self>()];
        Endianness::write_u64(&mut d[..8], self.rlim_cur);
        Endianness::write_u64(&mut d[8..], self.rlim_max);
        d
    }
}

// TODO: Write derive macro or something like this to automate the from trait?
impl<Endianness: ByteOrder> TryFrom<&[u8]>
for Rlimit<Endianness> {
    type Error = GenericError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != mem::size_of::<u64>() * 2 {
            Err(GenericError::new(GenericErrorKind::Unknown, "Invalid length"))
        } else {
            Ok(Rlimit {
                rlim_cur: Endianness::read_u64(&value[..8]),
                rlim_max: Endianness::read_u64(&value[8..]),
                _phantom: Default::default(),
            })
        }
    }
}

#[repr(u64)]
#[derive(FromPrimitive)]
enum RLIMIT {
    CPU = 0,
    FSIZE = 1,
    DATA = 2,
    STACK = 3,
    CORE = 4,
    RSS = 5,
    NPROC = 6,
    NOFILE = 7,
    MEMLOCK = 8,
    AS = 9,
    LOCKS = 10,
    SIGPENDING = 11,
    MSGQUEUE = 12,
    NICE = 13,
    RTPRIO = 14,
    RTTIME = 15,
    #[default]
    INVALID,
}

impl<'a, Endianness: ByteOrder, Arch: Architecture>
Restorable for LinuxKernel<Endianness, Arch> {
    fn snapshot(&mut self)
        -> Result<(), GenericError> {
        todo!()
    }

    fn restore_and_discard(&mut self)
        -> Result<(), GenericError> {
        todo!()
    }

    fn restore(&mut self)
        -> Result<(), GenericError> {
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

#[derive(Debug)]
#[repr(i64)]
pub enum LinuxKernelError {
    EPERM = -1,
    ENOENT = -2,
    ESRCH = -3,
    EINTR = -4,
    EIO = -5,
    ENXIO = -6,
    E2BIG = -7,
    ENOEXEC = -8,
    EBADF = -9,
    ECHILD = -10,
    EAGAIN = -11,
    ENOMEM = -12,
    EACCES = -13,
    EFAULT = -14,
    ENOTBLK = -15,
    EBUSY = -16,
    EEXIST = -17,
    EXDEV = -18,
    ENODEV = -19,
    ENOTDIR = -20,
    EISDIR = -21,
    EINVAL = -22,
    ENFILE = -23,
    EMFILE = -24,
    ENOTTY = -25,
    ETXTBSY = -26,
    EFBIG = -27,
    ENOSPC = -28,
    ESPIPE = -29,
    EROFS = -30,
    EMLINK = -31,
    EPIPE = -32,
    EDOM = -33,
    ERANGE = -34,
    EDEADLK = -35,
    ENAMETOOLONG = -36,
    ENOLCK = -37,
    ENOSYS = -38,
    ENOTEMPTY = -39,
    ELOOP = -40,
    ENOMSG = -42,
    EIDRM = -43,
    ECHRNG = -44,
    EL2NSYNC = -45,
    EL3HLT = -46,
    EL3RST = -47,
    ELNRNG = -48,
    EUNATCH = -49,
    ENOCSI = -50,
    EL2HLT = -51,
    EBADE = -52,
    EBADR = -53,
    EXFULL = -54,
    ENOANO = -55,
    EBADRQC = -56,
    EBADSLT = -57,
    EBFONT = -59,
    ENOSTR = -60,
    ENODATA = -61,
    ETIME = -62,
    ENOSR = -63,
    ENONET = -64,
    ENOPKG = -65,
    EREMOTE = -66,
    ENOLINK = -67,
    EADV = -68,
    ESRMNT = -69,
    ECOMM = -70,
    EPROTO = -71,
    EMULTIHOP = -72,
    EDOTDOT = -73,
    EBADMSG = -74,
    EOVERFLOW = -75,
    ENOTUNIQ = -76,
    EBADFD = -77,
    EREMCHG = -78,
    ELIBACC = -79,
    ELIBBAD = -80,
    ELIBSCN = -81,
    ELIBMAX = -82,
    ELIBEXEC = -83,
    EILSEQ = -84,
    ERESTART = -85,
    ESTRPIPE = -86,
    EUSERS = -87,
    ENOTSOCK = -88,
    EDESTADDRREQ = -89,
    EMSGSIZE = -90,
    EPROTOTYPE = -91,
    ENOPROTOOPT = -92,
    EPROTONOSUPPORT = -93,
    ESOCKTNOSUPPORT = -94,
    EOPNOTSUPP = -95,
    EPFNOSUPPORT = -96,
    EAFNOSUPPORT = -97,
    EADDRINUSE = -98,
    EADDRNOTAVAIL = -99,
    ENETDOWN = -100,
    ENETUNREACH = -101,
    ENETRESET = -102,
    ECONNABORTED = -103,
    ECONNRESET = -104,
    ENOBUFS = -105,
    EISCONN = -106,
    ENOTCONN = -107,
    ESHUTDOWN = -108,
    ETOOMANYREFS = -109,
    ETIMEDOUT = -110,
    ECONNREFUSED = -111,
    EHOSTDOWN = -112,
    EHOSTUNREACH = -113,
    EALREADY = -114,
    EINPROGRESS = -115,
    ESTALE = -116,
    EUCLEAN = -117,
    ENOTNAM = -118,
    ENAVAIL = -119,
    EISNAM = -120,
    EREMOTEIO = -121,
    EDQUOT = -122,
    ENOMEDIUM = -123,
    EMEDIUMTYPE = -124,
    ECANCELED = -125,
    ENOKEY = -126,
    EKEYEXPIRED = -127,
    EKEYREVOKED = -128,
    EKEYREJECTED = -129,
    EOWNERDEAD = -130,
    ENOTRECOVERABLE = -131,
    ERFKILL = -132,
    EHWPOISON = -133,
}

impl From<LinuxKernelError> for u64 {
    fn from(e: LinuxKernelError) -> Self {
        e as u64
    }
}

impl LinuxKernelError {
    pub const EWOULDBLOCK: Self = Self::EAGAIN;
    // Resource temporarily unavailable
    pub const ENOTSUP: Self = Self::EOPNOTSUPP;
    // Operation not supported
    pub const EDEADLOCK: Self = Self::EDEADLK; // Resource deadlock avoided

    fn describe(&self) -> &str {
        match self {
            LinuxKernelError::EPERM => "Operation not permitted",
            LinuxKernelError::ENOENT => "No such file or directory",
            LinuxKernelError::ESRCH => "No such process",
            LinuxKernelError::EINTR => "Interrupted system call",
            LinuxKernelError::EIO => "Input/output error",
            LinuxKernelError::ENXIO => "No such device or address",
            LinuxKernelError::E2BIG => "Argument list too long",
            LinuxKernelError::ENOEXEC => "Exec format error",
            LinuxKernelError::EBADF => "Bad file descriptor",
            LinuxKernelError::ECHILD => "No child processes",
            LinuxKernelError::EAGAIN => "Resource temporarily unavailable",
            LinuxKernelError::ENOMEM => "Cannot allocate memory",
            LinuxKernelError::EACCES => "Permission denied",
            LinuxKernelError::EFAULT => "Bad address",
            LinuxKernelError::ENOTBLK => "Block device required",
            LinuxKernelError::EBUSY => "Device or resource busy",
            LinuxKernelError::EEXIST => "File exists",
            LinuxKernelError::EXDEV => "Invalid cross-device link",
            LinuxKernelError::ENODEV => "No such device",
            LinuxKernelError::ENOTDIR => "Not a directory",
            LinuxKernelError::EISDIR => "Is a directory",
            LinuxKernelError::EINVAL => "Invalid argument",
            LinuxKernelError::ENFILE => "Too many open files in system",
            LinuxKernelError::EMFILE => "Too many open files",
            LinuxKernelError::ENOTTY => "Inappropriate ioctl for device",
            LinuxKernelError::ETXTBSY => "Text file busy",
            LinuxKernelError::EFBIG => "File too large",
            LinuxKernelError::ENOSPC => "No space left on device",
            LinuxKernelError::ESPIPE => "Illegal seek",
            LinuxKernelError::EROFS => "Read-only file system",
            LinuxKernelError::EMLINK => "Too many links",
            LinuxKernelError::EPIPE => "Broken pipe",
            LinuxKernelError::EDOM => "Numerical argument out of domain",
            LinuxKernelError::ERANGE => "Numerical result out of range",
            LinuxKernelError::EDEADLK => "Resource deadlock avoided",
            LinuxKernelError::ENAMETOOLONG => "File name too long",
            LinuxKernelError::ENOLCK => "No locks available",
            LinuxKernelError::ENOSYS => "Function not implemented",
            LinuxKernelError::ENOTEMPTY => "Directory not empty",
            LinuxKernelError::ELOOP => "Too many levels of symbolic links",
            LinuxKernelError::ENOMSG => "No message of desired type",
            LinuxKernelError::EIDRM => "Identifier removed",
            LinuxKernelError::ECHRNG => "Channel number out of range",
            LinuxKernelError::EL2NSYNC => "Level 2 not synchronized",
            LinuxKernelError::EL3HLT => "Level 3 halted",
            LinuxKernelError::EL3RST => "Level 3 reset",
            LinuxKernelError::ELNRNG => "Link number out of range",
            LinuxKernelError::EUNATCH => "Protocol driver not attached",
            LinuxKernelError::ENOCSI => "No CSI structure available",
            LinuxKernelError::EL2HLT => "Level 2 halted",
            LinuxKernelError::EBADE => "Invalid exchange",
            LinuxKernelError::EBADR => "Invalid request descriptor",
            LinuxKernelError::EXFULL => "Exchange full",
            LinuxKernelError::ENOANO => "No anode",
            LinuxKernelError::EBADRQC => "Invalid request code",
            LinuxKernelError::EBADSLT => "Invalid slot",
            LinuxKernelError::EBFONT => "Bad font file format",
            LinuxKernelError::ENOSTR => "Device not a stream",
            LinuxKernelError::ENODATA => "No data available",
            LinuxKernelError::ETIME => "Timer expired",
            LinuxKernelError::ENOSR => "Out of streams resources",
            LinuxKernelError::ENONET => "Machine is not on the network",
            LinuxKernelError::ENOPKG => "Package not installed",
            LinuxKernelError::EREMOTE => "Object is remote",
            LinuxKernelError::ENOLINK => "Link has been severed",
            LinuxKernelError::EADV => "Advertise error",
            LinuxKernelError::ESRMNT => "Srmount error",
            LinuxKernelError::ECOMM => "Communication error on send",
            LinuxKernelError::EPROTO => "Protocol error",
            LinuxKernelError::EMULTIHOP => "Multihop attempted",
            LinuxKernelError::EDOTDOT => "RFS specific error",
            LinuxKernelError::EBADMSG => "Bad message",
            LinuxKernelError::EOVERFLOW => "Value too large for defined data type",
            LinuxKernelError::ENOTUNIQ => "Name not unique on network",
            LinuxKernelError::EBADFD => "File descriptor in bad state",
            LinuxKernelError::EREMCHG => "Remote address changed",
            LinuxKernelError::ELIBACC => "Can not access a needed shared library",
            LinuxKernelError::ELIBBAD => "Accessing a corrupted shared library",
            LinuxKernelError::ELIBSCN => ".lib section in a.out corrupted",
            LinuxKernelError::ELIBMAX => "Attempting to link in too many shared libraries",
            LinuxKernelError::ELIBEXEC => "Cannot exec a shared library directly",
            LinuxKernelError::EILSEQ => "Invalid or incomplete multibyte or wide character",
            LinuxKernelError::ERESTART => "Interrupted system call should be restarted",
            LinuxKernelError::ESTRPIPE => "Streams pipe error",
            LinuxKernelError::EUSERS => "Too many users",
            LinuxKernelError::ENOTSOCK => "Socket operation on non-socket",
            LinuxKernelError::EDESTADDRREQ => "Destination address required",
            LinuxKernelError::EMSGSIZE => "Message too long",
            LinuxKernelError::EPROTOTYPE => "Protocol wrong type for socket",
            LinuxKernelError::ENOPROTOOPT => "Protocol not available",
            LinuxKernelError::EPROTONOSUPPORT => "Protocol not supported",
            LinuxKernelError::ESOCKTNOSUPPORT => "Socket type not supported",
            LinuxKernelError::EOPNOTSUPP => "Operation not supported",
            LinuxKernelError::EPFNOSUPPORT => "Protocol family not supported",
            LinuxKernelError::EAFNOSUPPORT => "Address family not supported by protocol",
            LinuxKernelError::EADDRINUSE => "Address already in use",
            LinuxKernelError::EADDRNOTAVAIL => "Cannot assign requested address",
            LinuxKernelError::ENETDOWN => "Network is down",
            LinuxKernelError::ENETUNREACH => "Network is unreachable",
            LinuxKernelError::ENETRESET => "Network dropped connection on reset",
            LinuxKernelError::ECONNABORTED => "Software caused connection abort",
            LinuxKernelError::ECONNRESET => "Connection reset by peer",
            LinuxKernelError::ENOBUFS => "No buffer space available",
            LinuxKernelError::EISCONN => "Transport endpoint is already connected",
            LinuxKernelError::ENOTCONN => "Transport endpoint is not connected",
            LinuxKernelError::ESHUTDOWN => "Cannot send after transport endpoint shutdown",
            LinuxKernelError::ETOOMANYREFS => "Too many references: cannot splice",
            LinuxKernelError::ETIMEDOUT => "Connection timed out",
            LinuxKernelError::ECONNREFUSED => "Connection refused",
            LinuxKernelError::EHOSTDOWN => "Host is down",
            LinuxKernelError::EHOSTUNREACH => "No route to host",
            LinuxKernelError::EALREADY => "Operation already in progress",
            LinuxKernelError::EINPROGRESS => "Operation now in progress",
            LinuxKernelError::ESTALE => "Stale file handle",
            LinuxKernelError::EUCLEAN => "Structure needs cleaning",
            LinuxKernelError::ENOTNAM => "Not a XENIX named type file",
            LinuxKernelError::ENAVAIL => "No XENIX semaphores available",
            LinuxKernelError::EISNAM => "Is a named type file",
            LinuxKernelError::EREMOTEIO => "Remote I/O error",
            LinuxKernelError::EDQUOT => "Disk quota exceeded",
            LinuxKernelError::ENOMEDIUM => "No medium found",
            LinuxKernelError::EMEDIUMTYPE => "Wrong medium type",
            LinuxKernelError::ECANCELED => "Operation canceled",
            LinuxKernelError::ENOKEY => "Required key not available",
            LinuxKernelError::EKEYEXPIRED => "Key has expired",
            LinuxKernelError::EKEYREVOKED => "Key has been revoked",
            LinuxKernelError::EKEYREJECTED => "Key was rejected by service",
            LinuxKernelError::EOWNERDEAD => "Owner died",
            LinuxKernelError::ENOTRECOVERABLE => "State not recoverable",
            LinuxKernelError::ERFKILL => "Operation not possible due to RF-kill",
            LinuxKernelError::EHWPOISON => "Memory page has hardware error",
        }
    }
}

impl From<CPUError> for LinuxKernelError {
    fn from(_: CPUError) -> Self {
        LinuxKernelError::EINVAL
    }
}

impl From<FSError> for LinuxKernelError {
    fn from(_: FSError) -> Self {
        LinuxKernelError::EINVAL
    }
}

impl From<MemoryError> for LinuxKernelError {
    fn from(me: MemoryError) -> Self {
        use MemoryError::*;
        match me {
            NotMapped => LinuxKernelError::EFAULT,
            _ => LinuxKernelError::EINVAL,
        }
    }
}

impl From<GenericError> for LinuxKernelError {
    fn from(e: GenericError) -> Self {
        match e.kind() {
            GenericErrorKind::CPUError(e) => (*e).into(),
            GenericErrorKind::FSError(e) => (*e).into(),
            GenericErrorKind::Unknown => { LinuxKernelError::EINVAL }
        }
    }
}