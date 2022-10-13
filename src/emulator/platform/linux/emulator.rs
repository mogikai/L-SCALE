use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::{CPU, CPUError, Hooks, MMU, RegisterX86};
use crate::emulator::abstractions::emulator::{Emulator, StopReason};
use crate::emulator::abstractions::errors::GenericError;
use crate::emulator::abstractions::restore::Restorable;
use crate::emulator::platform::linux::architecture::x86::X86;
use crate::emulator::platform::linux::kernel::{LinuxKernel, LinuxKernelError};

#[derive(Debug)]
pub enum
CreationError {
    ProgramHeaderErr,
    Unknown,
}

pub struct
LinuxX86<'a, CpuT, Endianness>
where CpuT: CPU + MMU + Hooks<'a>, Endianness: ByteOrder {
    binary_path: PathBuf,
    cpu: Rc<RefCell<CpuT>>,
    kernel: Rc<RefCell<LinuxKernel<Endianness, X86>>>,
    phantom: PhantomData<&'a CpuT>,
}

impl<'a, CpuT, Endianness>
Restorable for
LinuxX86<'a, CpuT, Endianness>
where CpuT: CPU + MMU + Hooks<'a>, Endianness: ByteOrder {
    fn snapshot(
        &mut self
    ) -> Result<(), GenericError> {
        self.kernel.borrow_mut().snapshot()?;
        self.cpu.borrow_mut().snapshot()
    }

    fn restore_and_discard(
        &mut self
    ) -> Result<(), GenericError> {
        self.kernel.borrow_mut().restore_and_discard()?;
        self.cpu.borrow_mut().restore_and_discard()
    }

    fn restore(
        &mut self
    ) -> Result<(), GenericError> {
        self.kernel.borrow_mut().restore()?;
        self.cpu.borrow_mut().restore()
    }
}

impl
From<CPUError> for
StopReason {
    fn from(
        _: CPUError
    ) -> Self {
        StopReason::Unknown
    }
}

impl<'a, CpuT, Endianness>
Emulator for
LinuxX86<'a, CpuT, Endianness>
where CpuT: CPU + MMU + Hooks<'a>,
      Endianness: ByteOrder {
    fn run(
        &mut self
    ) -> Result<(), StopReason> {
        self.cpu.borrow_mut().emulate(None, None, None)?;
        Ok(())
    }
}

macro_rules! do_syscall {
	(0, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			$debug_str = format!("() ");
			$name.$fun($uc)
		}
	};
	(1, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			$debug_str = format!("(0x{:X}) ", rdi);
			$name.$fun($uc, rdi)
		}
	};
	(2, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}) ", rdi, rsi);
			$name.$fun($uc, rdi, rsi)
		}
	};
	(3, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx);
			$name.$fun($uc, rdi, rsi, rdx)
		}};
	(4, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10);
			$name.$fun($uc, rdi, rsi, rdx, r10)
		}
	};
    (5, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
            let r8 = $uc.reg_read(RegisterX86::R8.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10, r8);
			$name.$fun($uc, rdi, rsi, rdx, r10, r8)
		}
	};
	(6, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
			let r8 = $uc.reg_read(RegisterX86::R8.into()).unwrap();
			let r9 = $uc.reg_read(RegisterX86::R9.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10, r8, r9);
			$name.$fun($uc, rdi, rsi, rdx, r10, r8, r9)
		}
	}
}

#[repr(u64)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
#[derive(Debug, FromPrimitive)]
enum SYSCALL {
    READ = 0,
    WRITE = 1,
    CLOSE = 3,
    STAT = 4,
    FSTAT = 5,
    MMAP = 9,
    MPROTECT = 10,
    MUNMAP = 11,
    BRK = 12,
    RT_SIGACTION = 13,
    RT_SIGPROCMASK = 14,
    PREAD64 = 17,
    WRITEV = 20,
    ACCESS = 21,
    MREMAP = 25,
    GETPID = 39,
    SOCKET = 41,
    CONNECT = 42,
    ACCEPT = 43,
    SENDTO = 44,
    RECVFROM = 45,
    SENDMSG = 46,
    RECVMSG = 47,
    SHUTDOWN = 48,
    BIND = 49,
    LISTEN = 50,
    GETSOCKNAME = 51,
    GETPEERNAME = 52,
    SOCKETPAIR = 53,
    SETSOCKOPT = 54,
    GETSOCKOPT = 55,
    UNAME = 63,
    GETTIMEOFDAY = 96,
    ARCH_PRCTL = 158,
    TIME = 201,
    EPOLL_CREATE = 213,
    SET_TID_ADDRESS = 218,
    CLOCK_GETTIME = 228,
    EPOLL_WAIT = 232,
    EPOLL_CTL = 233,
    OPENAT = 257,
    SET_ROBUST_LIST = 273,
    EPOLL_PWAIT = 281,
    EPOLL_CREATE1 = 291,
    PRLIMIT64 = 302,
    #[default]
    NONE = !0,
}

fn syscall<Endianness: ByteOrder + 'static>(
    cpu: &mut (impl CPU + MMU),
    kernel: &mut LinuxKernel<Endianness, X86>,
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
        UNAME => do_syscall!(1, cpu, kernel::uname, debug_str),
        GETTIMEOFDAY => do_syscall!(2, cpu, kernel::gettimeofday, debug_str),
        ARCH_PRCTL => do_syscall!(2, cpu, kernel::arch_prctl, debug_str),
        TIME => do_syscall!(1, cpu, kernel::time, debug_str),
        EPOLL_CREATE => do_syscall!(1, cpu, kernel::epoll_create, debug_str),
        SET_TID_ADDRESS => do_syscall!(1, cpu, kernel::set_tid_address, debug_str),
        CLOCK_GETTIME => do_syscall!(2, cpu, kernel::clock_gettime, debug_str),
        EPOLL_WAIT => do_syscall!(4, cpu, kernel::epoll_wait, debug_str),
        EPOLL_CTL => do_syscall!(4, cpu, kernel::epoll_ctl, debug_str),
        OPENAT => do_syscall!(4, cpu, kernel::openat, debug_str),
        SET_ROBUST_LIST => do_syscall!(2, cpu, kernel::set_robust_list, debug_str),
        EPOLL_PWAIT => do_syscall!(6, cpu, kernel::epoll_pwait, debug_str),
        EPOLL_CREATE1 => do_syscall!(1, cpu, kernel::epoll_create1, debug_str),
        PRLIMIT64 => do_syscall!(4, cpu, kernel::prlimit64, debug_str),
        NONE => {
            debug_str = format!("(rax: {})", rax);
            unknown_syscall = true;
            Err(LinuxKernelError::EINVAL)
        }
    };
    let rax = match result {
        Ok(rax) => rax,
        Err(e) => e.into(),
    };
    if unknown_syscall {
        error!("SYSCALL: {:?} {} -> 0x{:X}", syscall, debug_str, rax);
        cpu.kill().unwrap();
    } else {
        trace!("SYSCALL: {:?} {} -> 0x{:X}", syscall, debug_str, rax);
    }
    cpu.reg_write(RegisterX86::RAX.into(), rax).unwrap();
}

impl<'a, CpuT, Endianness>
LinuxX86<'a, CpuT, Endianness>
where CpuT: CPU + MMU + Hooks<'a>,
      Endianness: ByteOrder + 'static {
    pub
    fn new(
        kernel: Rc<RefCell<LinuxKernel<Endianness, X86>>>,
        cpu: Rc<RefCell<CpuT>>,
        binary_path: PathBuf,
    ) -> Result<LinuxX86<'a, CpuT, Endianness>, CreationError> {
        let emu = LinuxX86 {
            binary_path,
            cpu,
            kernel: kernel.clone(),
            phantom: Default::default(),
        };
        kernel.borrow_mut().add_stdio_handles();

        let mut cpu = emu.cpu.borrow_mut();
        kernel.borrow_mut().load_binary(&mut *cpu, emu.binary_path.as_path()).unwrap();
        let kernel_copy = kernel.clone();
        cpu.add_block_hook(Box::new(move |_, addr, instrs| {
            kernel_copy.borrow_mut().do_timestep(instrs);
        }));

        let cb = move |cpu: &mut CpuT| {
            syscall(cpu, &mut *kernel.borrow_mut());
        };
        cpu.add_syscall_hook(1, 0, Box::new(cb));
        drop(cpu);
        Ok(emu)
    }
}
