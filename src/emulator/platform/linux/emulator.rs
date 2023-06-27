use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::address::EVAddr;
use crate::emulator::abstractions::cpu::{CPUError, ContextHandle, RegisterX86, CPU};
use crate::emulator::abstractions::emulator::{Emulator, StopReason};
use crate::emulator::abstractions::errors::{GenericError, GenericErrorKind};
use crate::emulator::abstractions::hooks::Hooks;
use crate::emulator::abstractions::mmu::{MemoryError, MMU};
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::task_id::TaskId;
use crate::emulator::platform::linux::kernel::{LinuxKernelError, LinuxKernelTask};
use crate::emulator::system::System;

macro_rules! do_syscall {
    (0, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        $debug_str = format!("() ");
        $name.$fun($uc)
    }};
    (1, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        $debug_str = format!("(0x{:X}) ", rdi);
        $name.$fun($uc, rdi.into())
    }};
    (2, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
        $debug_str = format!("(0x{:X}, 0x{:X}) ", rdi, rsi);
        $name.$fun($uc, rdi.into(), rsi.into())
    }};
    (3, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
        let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
        $debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx);
        $name.$fun($uc, rdi.into(), rsi.into(), rdx.into())
    }};
    (4, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
        let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
        let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
        $debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10);
        $name.$fun($uc, rdi.into(), rsi.into(), rdx.into(), r10.into())
    }};
    (5, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
        let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
        let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
        let r8 = $uc.reg_read(RegisterX86::R8.into()).unwrap();
        $debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10, r8);
        $name.$fun($uc, rdi.into(), rsi.into(), rdx.into(), r10.into(), r8.into())
    }};
    (6, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
        let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
        let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
        let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
        let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
        let r8 = $uc.reg_read(RegisterX86::R8.into()).unwrap();
        let r9 = $uc.reg_read(RegisterX86::R9.into()).unwrap();
        $debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10, r8, r9);
        $name.$fun($uc, rdi.into(), rsi.into(), rdx.into(), r10.into(), r8.into(), r9.into())
    }};
}

#[derive(Debug)]
pub enum CreationError {
    ProgramHeaderErr,
    Unknown,
}

struct SchedulerContext<Endianness: ByteOrder> {
    tasks: BTreeMap<TaskId, LinuxKernelTask<Endianness, X86>>,
}

pub struct LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder,
{
    system: Rc<System<Endianness>>,
    context: Rc<RefCell<SchedulerContext<Endianness>>>,
    cpu: Rc<RefCell<CpuT>>,
    cpu_context_handle: ContextHandle,
    phantom: PhantomData<&'a CpuT>,
}

// impl<'a, CpuT, Endianness> Restorable for LinuxX86Scheduler<'a, CpuT, Endianness>
// where
//     CpuT: CPU + MMU + Hooks<'a>,
//     Endianness: ByteOrder,
// {
//     fn snapshot(&mut self) -> Result<(), GenericError> {
//         self.kernel.snapshot()?;
//         self.cpu.borrow_mut().snapshot()
//     }
//
//     fn restore_and_discard(&mut self) -> Result<(), GenericError> {
//         self.kernel.restore_and_discard()?;
//         self.cpu.borrow_mut().restore_and_discard()
//     }
//
//     fn restore(&mut self) -> Result<(), GenericError> {
//         self.kernel.restore()?;
//         self.cpu.borrow_mut().restore()
//     }
// }

impl From<CPUError> for StopReason {
    fn from(cpuerror: CPUError) -> Self {
        match cpuerror {
            CPUError::Stopped(stop_reason) => stop_reason,
            _ => StopReason::Unknown,
        }
    }
}

impl<'a, CpuT, Endianness> Emulator for LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    fn run(
        &mut self,
        timeout: Option<Duration>,
        instruction_count: Option<usize>,
    ) -> Result<(), StopReason> {
        // TODO: Implement scheduling
        // let mut processes = vec![scheduler];
        //
        // loop {
        //     processes.retain_mut(|process| {
        //         let result = process.run(None, Some(10000));
        //         result.is_ok()
        //     });
        //
        //     if processes.is_empty() {
        //         break;
        //     }
        // }
        if let Some(current_task) = *self.system.current_task.borrow() {
            let mut cpu = self.cpu.borrow_mut();
            cpu.restore_context(self.cpu_context_handle)?;
            cpu.emulate(None, timeout, instruction_count)?;
            cpu.save_context(self.cpu_context_handle)?;
            let inner = self.context.borrow();
            if inner.tasks.get(&current_task).unwrap().exited() {
                Err(StopReason::ProcessExit)
            } else {
                Ok(())
            }
        } else {
            Err(StopReason::Unknown)
        }
    }
}

fn syscall<Endianness: ByteOrder + 'static>(
    cpu: &mut (impl CPU + MMU),
    kernel: &mut LinuxKernelTask<Endianness, X86>,
) {
    let rax = cpu.reg_read(RegisterX86::RAX.into()).unwrap();
    let mut debug_str = String::new();
    let syscall = SYSCALL::from(rax);
    let mut unknown_syscall = false;
    use SYSCALL::*;
    let result = match SYSCALL::from(rax) {
        READ => do_syscall!(3, cpu, kernel::read, debug_str),
        WRITE => do_syscall!(3, cpu, kernel::write, debug_str),
        CLOSE => do_syscall!(1, cpu, kernel::close, debug_str),
        STAT => do_syscall!(2, cpu, kernel::stat, debug_str),
        FSTAT => do_syscall!(2, cpu, kernel::fstat, debug_str),
        MMAP => do_syscall!(6, cpu, kernel::mmap, debug_str),
        MPROTECT => do_syscall!(3, cpu, kernel::mprotect, debug_str),
        MUNMAP => do_syscall!(2, cpu, kernel::munmap, debug_str),
        BRK => do_syscall!(1, cpu, kernel::brk, debug_str),
        RT_SIGACTION => do_syscall!(4, cpu, kernel::rt_sigaction, debug_str),
        RT_SIGPROCMASK => do_syscall!(4, cpu, kernel::rt_sigprocmask, debug_str),
        IOCTL => do_syscall!(3, cpu, kernel::ioctl, debug_str),
        PREAD64 => do_syscall!(4, cpu, kernel::pread64, debug_str),
        WRITEV => do_syscall!(3, cpu, kernel::writev, debug_str),
        ACCESS => do_syscall!(2, cpu, kernel::access, debug_str),
        MREMAP => do_syscall!(5, cpu, kernel::mremap, debug_str),
        GETPID => do_syscall!(0, cpu, kernel::getpid, debug_str),
        SOCKET => do_syscall!(3, cpu, kernel::socket, debug_str),
        CONNECT => do_syscall!(3, cpu, kernel::connect, debug_str),
        ACCEPT => do_syscall!(3, cpu, kernel::accept, debug_str),
        SENDTO => do_syscall!(6, cpu, kernel::sendto, debug_str),
        RECVFROM => do_syscall!(6, cpu, kernel::recvfrom, debug_str),
        SENDMSG => do_syscall!(3, cpu, kernel::sendmsg, debug_str),
        RECVMSG => do_syscall!(3, cpu, kernel::recvmsg, debug_str),
        SHUTDOWN => do_syscall!(2, cpu, kernel::shutdown, debug_str),
        BIND => do_syscall!(3, cpu, kernel::bind, debug_str),
        LISTEN => do_syscall!(2, cpu, kernel::listen, debug_str),
        GETSOCKNAME => do_syscall!(3, cpu, kernel::getsockname, debug_str),
        GETPEERNAME => do_syscall!(3, cpu, kernel::getpeername, debug_str),
        SOCKETPAIR => do_syscall!(4, cpu, kernel::socketpair, debug_str),
        SETSOCKOPT => do_syscall!(5, cpu, kernel::setsockopt, debug_str),
        GETSOCKOPT => do_syscall!(5, cpu, kernel::getsockopt, debug_str),
        EXECVE => do_syscall!(3, cpu, kernel::execve, debug_str),
        UNAME => do_syscall!(1, cpu, kernel::uname, debug_str),
        FCNTL => do_syscall!(3, cpu, kernel::fcntl, debug_str),
        GETTIMEOFDAY => do_syscall!(2, cpu, kernel::gettimeofday, debug_str),
        STATFS => do_syscall!(2, cpu, kernel::statfs, debug_str),
        ARCH_PRCTL => do_syscall!(2, cpu, kernel::arch_prctl, debug_str),
        GETTID => do_syscall!(0, cpu, kernel::gettid, debug_str),
        TIME => do_syscall!(1, cpu, kernel::time, debug_str),
        EPOLL_CREATE => do_syscall!(1, cpu, kernel::epoll_create, debug_str),
        SET_TID_ADDRESS => do_syscall!(1, cpu, kernel::set_tid_address, debug_str),
        FADVISE64 => do_syscall!(4, cpu, kernel::fadvise64, debug_str),
        CLOCK_GETTIME => do_syscall!(2, cpu, kernel::clock_gettime, debug_str),
        EXIT_GROUP => {
            cpu.stop(StopReason::ProcessExit).unwrap();
            Ok(0)
        } // TODO: Implement properly and propagate exit code
        EPOLL_WAIT => do_syscall!(4, cpu, kernel::epoll_wait, debug_str),
        EPOLL_CTL => do_syscall!(4, cpu, kernel::epoll_ctl, debug_str),
        OPENAT => do_syscall!(4, cpu, kernel::openat, debug_str),
        NEWFSTATAT => do_syscall!(4, cpu, kernel::newfstatat, debug_str),
        SET_ROBUST_LIST => do_syscall!(2, cpu, kernel::set_robust_list, debug_str),
        EPOLL_PWAIT => do_syscall!(6, cpu, kernel::epoll_pwait, debug_str),
        SIGNALFD4 => do_syscall!(4, cpu, kernel::signalfd4, debug_str),
        EPOLL_CREATE1 => do_syscall!(1, cpu, kernel::epoll_create1, debug_str),
        PRLIMIT64 => do_syscall!(4, cpu, kernel::prlimit64, debug_str),
        GETRANDOM => do_syscall!(3, cpu, kernel::getrandom, debug_str),
        RSEQ => do_syscall!(4, cpu, kernel::rseq, debug_str),
        _ => {
            debug_str = format!("(rax: {})", rax);
            unknown_syscall = true;
            Err(LinuxKernelError::EINVAL)
        }
    };
    let rax = match result {
        Ok(rax) => {
            debug_str = format!("SYSCALL: {:?} {} -> 0x{:X}", syscall, debug_str, rax);
            rax
        }
        Err(e) => {
            debug_str = format!("SYSCALL: {:?} {} -> 0x{:X} ({})", syscall, debug_str, u64::from(e), e.describe());
            e.into()
        }
    };
    if unknown_syscall {
        error!("SYSCALL NOT IMPLEMENTED: {}", debug_str);
        cpu.stop(StopReason::MissingSyscall).unwrap();
    } else {
        debug!("{}", debug_str);
    }
    cpu.reg_write(RegisterX86::RAX.into(), rax).unwrap();
}

impl<'a, CpuT, Endianness> LinuxX86Scheduler<'a, CpuT, Endianness>
where
    CpuT: CPU + MMU + Hooks<'a>,
    Endianness: ByteOrder + 'static,
{
    pub fn new(
        system: Rc<System<Endianness>>,
        initial_task: LinuxKernelTask<Endianness, X86>,
        cpu: Rc<RefCell<CpuT>>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<LinuxX86Scheduler<'a, CpuT, Endianness>, GenericError> {
        let cpu_context_handle = cpu.borrow_mut().create_context()?;
        let mut sched = LinuxX86Scheduler {
            system,
            context: Rc::new(RefCell::new(SchedulerContext {
                tasks: Default::default(),
            })),
            cpu,
            cpu_context_handle,
            phantom: Default::default(),
        };

        sched.setup_initial_task(initial_task, binary_path, binary_args, binary_env)?;

        sched.setup_cpu()?;

        Ok(sched)
    }

    fn setup_cpu(&mut self) -> Result<(), GenericError> {
        let mut cpu = self.cpu.borrow_mut();

        // TODO: Potentially move this to the clock setup?
        let timestep_system = self.system.clone();
        cpu.add_block_hook(Box::new(move |_, _, instrs| {
            timestep_system.clock.borrow_mut().do_timestep(instrs);
        }));

        let syscall_context = self.context.clone();
        let syscall_system = self.system.clone();
        cpu.add_syscall_hook(
            EVAddr::from(1),
            EVAddr::from(0),
            Box::new(move |cpu: &mut CpuT| {
                let mut context = syscall_context.borrow_mut();
                let current_task = syscall_system.current_task.borrow().unwrap();
                let task = context.tasks.get_mut(&current_task).unwrap();
                syscall(cpu, task);
            }),
        );

        cpu.add_unmapped_mem_hook(
            1.into(),
            0.into(),
            Box::new(move |_, action, addr, size, _| {
                trace!("{:?} on {:#X} with size {} ", action, addr, size);
                false
            }),
        );

        cpu.save_context(self.cpu_context_handle)?;
        Ok(())
    }

    fn setup_initial_task(
        &mut self,
        mut initial_task: LinuxKernelTask<Endianness, X86>,
        binary_path: PathBuf,
        binary_args: &[&str],
        binary_env: &[&str],
    ) -> Result<(), GenericError> {
        initial_task.add_stdio_handles();

        let mut cpu = self.cpu.borrow_mut();
        initial_task.load_binary(&mut *cpu, binary_path.as_path(), binary_args, binary_env)?;
        *self.system.current_task.borrow_mut() = Some(initial_task.task_id());
        let mut context = self.context.borrow_mut();
        context
            .tasks
            .try_insert(initial_task.task_id(), initial_task)
            .map_err(|_| GenericError::from(CPUError::TaskExists))?;
        Ok(())
    }
}

#[repr(u64)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
enum SYSCALL {
    READ               = 0,
    WRITE              = 1,
    OPEN               = 2,
    CLOSE              = 3,
    STAT               = 4,
    FSTAT              = 5,
    LSTAT              = 6,
    POLL               = 7,
    LSEEK              = 8,
    MMAP               = 9,
    MPROTECT           = 10,
    MUNMAP             = 11,
    BRK                = 12,
    RT_SIGACTION       = 13,
    RT_SIGPROCMASK     = 14,
    RT_SIGRETURN       = 15,
    IOCTL              = 16,
    PREAD64            = 17,
    PWRITE64           = 18,
    READV              = 19,
    WRITEV             = 20,
    ACCESS             = 21,
    PIPE               = 22,
    SELECT             = 23,
    SCHED_YIELD        = 24,
    MREMAP             = 25,
    MSYNC              = 26,
    MINCORE            = 27,
    MADVISE            = 28,
    SHMGET             = 29,
    SHMAT              = 30,
    SHMCTL             = 31,
    DUP                = 32,
    DUP2               = 33,
    PAUSE              = 34,
    NANOSLEEP          = 35,
    GETITIMER          = 36,
    ALARM              = 37,
    SETITIMER          = 38,
    GETPID             = 39,
    SENDFILE           = 40,
    SOCKET             = 41,
    CONNECT            = 42,
    ACCEPT             = 43,
    SENDTO             = 44,
    RECVFROM           = 45,
    SENDMSG            = 46,
    RECVMSG            = 47,
    SHUTDOWN           = 48,
    BIND               = 49,
    LISTEN             = 50,
    GETSOCKNAME        = 51,
    GETPEERNAME        = 52,
    SOCKETPAIR         = 53,
    SETSOCKOPT         = 54,
    GETSOCKOPT         = 55,
    CLONE              = 56,
    FORK               = 57,
    VFORK              = 58,
    EXECVE             = 59,
    EXIT               = 60,
    WAIT4              = 61,
    KILL               = 62,
    UNAME              = 63,
    SEMGET             = 64,
    SEMOP              = 65,
    SEMCTL             = 66,
    SHMDT              = 67,
    MSGGET             = 68,
    MSGSND             = 69,
    MSGRCV             = 70,
    MSGCTL             = 71,
    FCNTL              = 72,
    FLOCK              = 73,
    FSYNC              = 74,
    FDATASYNC          = 75,
    TRUNCATE           = 76,
    FTRUNCATE          = 77,
    GETDENTS           = 78,
    GETCWD             = 79,
    CHDIR              = 80,
    FCHDIR             = 81,
    RENAME             = 82,
    MKDIR              = 83,
    RMDIR              = 84,
    CREAT              = 85,
    LINK               = 86,
    UNLINK             = 87,
    SYMLINK            = 88,
    READLINK           = 89,
    CHMOD              = 90,
    FCHMOD             = 91,
    CHOWN              = 92,
    FCHOWN             = 93,
    LCHOWN             = 94,
    UMASK              = 95,
    GETTIMEOFDAY       = 96,
    GETRLIMIT          = 97,
    GETRUSAGE          = 98,
    SYSINFO            = 99,
    TIMES              = 100,
    PTRACE             = 101,
    GETUID             = 102,
    SYSLOG             = 103,
    GETGID             = 104,
    SETUID             = 105,
    SETGID             = 106,
    GETEUID            = 107,
    GETEGID            = 108,
    SETPGID            = 109,
    GETPPID            = 110,
    GETPGRP            = 111,
    SETSID             = 112,
    SETREUID           = 113,
    SETREGID           = 114,
    GETGROUPS          = 115,
    SETGROUPS          = 116,
    SETRESUID          = 117,
    GETRESUID          = 118,
    SETRESGID          = 119,
    GETRESGID          = 120,
    GETPGID            = 121,
    SETFSUID           = 122,
    SETFSGID           = 123,
    GETSID             = 124,
    CAPGET             = 125,
    CAPSET             = 126,
    RT_SIGPENDING      = 127,
    RT_SIGTIMEDWAIT    = 128,
    RT_SIGQUEUEINFO    = 129,
    RT_SIGSUSPEND      = 130,
    SIGALTSTACK        = 131,
    UTIME              = 132,
    MKNOD              = 133,
    USELIB             = 134,
    PERSONALITY        = 135,
    USTAT              = 136,
    STATFS             = 137,
    FSTATFS            = 138,
    SYSFS              = 139,
    GETPRIORITY        = 140,
    SETPRIORITY        = 141,
    SCHED_SETPARAM     = 142,
    SCHED_GETPARAM     = 143,
    SCHED_SETSCHEDULER = 144,
    SCHED_GETSCHEDULER = 145,
    SCHED_GET_PRIORITY_MAX = 146,
    SCHED_GET_PRIORITY_MIN = 147,
    SCHED_RR_GET_INTERVAL = 148,
    MLOCK              = 149,
    MUNLOCK            = 150,
    MLOCKALL           = 151,
    MUNLOCKALL         = 152,
    VHANGUP            = 153,
    MODIFY_LDT         = 154,
    PIVOT_ROOT         = 155,
    SYSCTL             = 156,
    PRCTL              = 157,
    ARCH_PRCTL         = 158,
    ADJTIMEX           = 159,
    SETRLIMIT          = 160,
    CHROOT             = 161,
    SYNC               = 162,
    ACCT               = 163,
    SETTIMEOFDAY       = 164,
    MOUNT              = 165,
    UMOUNT2            = 166,
    SWAPON             = 167,
    SWAPOFF            = 168,
    REBOOT             = 169,
    SETHOSTNAME        = 170,
    SETDOMAINNAME      = 171,
    IOPL               = 172,
    IOPERM             = 173,
    CREATE_MODULE      = 174,
    INIT_MODULE        = 175,
    DELETE_MODULE      = 176,
    GET_KERNEL_SYMS    = 177,
    QUERY_MODULE       = 178,
    QUOTACTL           = 179,
    NFSSERVCTL         = 180,
    GETPMSG            = 181,
    PUTPMSG            = 182,
    AFS_SYSCALL        = 183,
    TUXCALL            = 184,
    SECURITY           = 185,
    GETTID             = 186,
    READAHEAD          = 187,
    SETXATTR           = 188,
    LSETXATTR          = 189,
    FSETXATTR          = 190,
    GETXATTR           = 191,
    LGETXATTR          = 192,
    FGETXATTR          = 193,
    LISTXATTR          = 194,
    LLISTXATTR         = 195,
    FLISTXATTR         = 196,
    REMOVEXATTR        = 197,
    LREMOVEXATTR       = 198,
    FREMOVEXATTR       = 199,
    TKILL              = 200,
    TIME               = 201,
    FUTEX              = 202,
    SCHED_SETAFFINITY  = 203,
    SCHED_GETAFFINITY  = 204,
    SET_THREAD_AREA    = 205,
    IO_SETUP           = 206,
    IO_DESTROY         = 207,
    IO_GETEVENTS       = 208,
    IO_SUBMIT          = 209,
    IO_CANCEL          = 210,
    GET_THREAD_AREA    = 211,
    LOOKUP_DCOOKIE     = 212,
    EPOLL_CREATE       = 213,
    EPOLL_CTL_OLD      = 214,
    EPOLL_WAIT_OLD     = 215,
    REMAP_FILE_PAGES   = 216,
    GETDENTS64         = 217,
    SET_TID_ADDRESS    = 218,
    RESTART_SYSCALL    = 219,
    SEMTIMEDOP         = 220,
    FADVISE64          = 221,
    TIMER_CREATE       = 222,
    TIMER_SETTIME      = 223,
    TIMER_GETTIME      = 224,
    TIMER_GETOVERRUN   = 225,
    TIMER_DELETE       = 226,
    CLOCK_SETTIME      = 227,
    CLOCK_GETTIME      = 228,
    CLOCK_GETRES       = 229,
    CLOCK_NANOSLEEP    = 230,
    EXIT_GROUP         = 231,
    EPOLL_WAIT         = 232,
    EPOLL_CTL          = 233,
    TGKILL             = 234,
    UTIMES             = 235,
    VSERVER            = 236,
    MBIND              = 237,
    SET_MEMPOLICY      = 238,
    GET_MEMPOLICY      = 239,
    MQ_OPEN            = 240,
    MQ_UNLINK          = 241,
    MQ_TIMEDSEND       = 242,
    MQ_TIMEDRECEIVE    = 243,
    MQ_NOTIFY          = 244,
    MQ_GETSETATTR      = 245,
    KEXEC_LOAD         = 246,
    WAITID             = 247,
    ADD_KEY            = 248,
    REQUEST_KEY        = 249,
    KEYCTL             = 250,
    IOPRIO_SET         = 251,
    IOPRIO_GET         = 252,
    INOTIFY_INIT       = 253,
    INOTIFY_ADD_WATCH  = 254,
    INOTIFY_RM_WATCH   = 255,
    MIGRATE_PAGES      = 256,
    OPENAT             = 257,
    MKDIRAT            = 258,
    MKNODAT            = 259,
    FCHOWNAT           = 260,
    FUTIMESAT          = 261,
    NEWFSTATAT         = 262,
    UNLINKAT           = 263,
    RENAMEAT           = 264,
    LINKAT             = 265,
    SYMLINKAT          = 266,
    READLINKAT         = 267,
    FCHMODAT           = 268,
    FACCESSAT          = 269,
    PSELECT6           = 270,
    PPOLL              = 271,
    UNSHARE            = 272,
    SET_ROBUST_LIST    = 273,
    GET_ROBUST_LIST    = 274,
    SPLICE             = 275,
    TEE                = 276,
    SYNC_FILE_RANGE    = 277,
    VMSPLICE           = 278,
    MOVE_PAGES         = 279,
    UTIMENSAT          = 280,
    EPOLL_PWAIT        = 281,
    SIGNALFD           = 282,
    TIMERFD_CREATE     = 283,
    EVENTFD            = 284,
    FALLOCATE          = 285,
    TIMERFD_SETTIME    = 286,
    TIMERFD_GETTIME    = 287,
    ACCEPT4            = 288,
    SIGNALFD4          = 289,
    EVENTFD2           = 290,
    EPOLL_CREATE1      = 291,
    DUP3               = 292,
    PIPE2              = 293,
    INOTIFY_INIT1      = 294,
    PREADV             = 295,
    PWRITEV            = 296,
    RT_TGSIGQUEUEINFO  = 297,
    PERF_EVENT_OPEN    = 298,
    RECVMMSG           = 299,
    FANOTIFY_INIT      = 300,
    FANOTIFY_MARK      = 301,
    PRLIMIT64          = 302,
    NAME_TO_HANDLE_AT  = 303,
    OPEN_BY_HANDLE_AT  = 304,
    CLOCK_ADJTIME      = 305,
    SYNCFS             = 306,
    SENDMMSG           = 307,
    SETNS              = 308,
    GETCPU             = 309,
    PROCESS_VM_READV   = 310,
    PROCESS_VM_WRITEV  = 311,
    KCMP               = 312,
    FINIT_MODULE       = 313,
    SCHED_SETATTR      = 314,
    SCHED_GETATTR      = 315,
    RENAMEAT2          = 316,
    SECCOMP            = 317,
    GETRANDOM          = 318,
    MEMFD_CREATE       = 319,
    KEXEC_FILE_LOAD    = 320,
    BPF                = 321,
    EXECVEAT           = 322,
    USERFAULTFD        = 323,
    MEMBARRIER         = 324,
    MLOCK2             = 325,
    COPY_FILE_RANGE    = 326,
    PREADV2            = 327,
    PWRITEV2           = 328,
    PKEY_MPROTECT      = 329,
    PKEY_ALLOC         = 330,
    PKEY_FREE          = 331,
    STATX              = 332,
    IO_PGETEVENTS      = 333,
    RSEQ               = 334,
    PIDFD_SEND_SIGNAL  = 424,
    IO_URING_SETUP     = 425,
    IO_URING_ENTER     = 426,
    IO_URING_REGISTER  = 427,
    OPEN_TREE          = 428,
    MOVE_MOUNT         = 429,
    FSOPEN             = 430,
    FSCONFIG           = 431,
    FSMOUNT            = 432,
    FSPICK             = 433,
    PIDFD_OPEN         = 434,
    CLONE3             = 435,
    CLOSE_RANGE        = 436,
    OPENAT2            = 437,
    PIDFD_GETFD        = 438,
    FACCESSAT2         = 439,
    PROCESS_MADVISE    = 440,
    EPOLL_PWAIT2       = 441,
    MOUNT_SETATTR      = 442,
    QUOTACTL_FD        = 443,
    LANDLOCK_CREATE_RULESET = 444,
    LANDLOCK_ADD_RULE  = 445,
    LANDLOCK_RESTRICT_SELF = 446,
    MEMFD_SECRET       = 447,
    PROCESS_MRELEASE   = 448,
    #[default]
    NONE               = !0,
}
