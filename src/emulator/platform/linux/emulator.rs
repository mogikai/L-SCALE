use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use byteorder::ByteOrder;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::{CPU, CPUError, Hooks, MMU, RegisterX86};
use crate::emulator::abstractions::emulator::{Emulator, StopReason};
use crate::emulator::platform::linux::kernel::LinuxKernel;
use crate::emulator::platform::linux::architecture::x86::X86;

#[derive(Debug)]
pub enum CreationError {
    ProgramHeaderErr,
    Unknown,
}

pub struct LinuxX86<'a, CpuT, Endianness> where CpuT: CPU + MMU + Hooks<'a>, Endianness: ByteOrder {
    binary_path: PathBuf,
    cpu: Rc<RefCell<CpuT>>,
    kernel: Rc<RefCell<LinuxKernel<Endianness, X86>>>,
    phantom: PhantomData<&'a CpuT>,
}

impl From<CPUError> for StopReason {
    fn from(_: CPUError) -> Self {
        StopReason::Unknown
    }
}

impl<'a, CpuT, Endianness> Emulator for LinuxX86<'a, CpuT, Endianness> where CpuT: CPU + MMU + Hooks<'a>, Endianness: ByteOrder {
    fn run(&mut self) -> Result<(), StopReason> {
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
	(3, $uc:ident, $name:ident::$fun:ident, $debug_str:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			$debug_str = format!("(0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx);
			$name.$fun($uc, rdi, rsi, rdx)
		}
	};
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
    BRK = 12,
    RT_SIGACTION = 13,
    RT_SIGPROCMASK = 14,
    PREAD64 = 17,
    WRITEV = 20,
    ACCESS = 21,
    GETPID = 39,
    UNAME = 63,
    GETTIMEOFDAY = 96,
    ARCH_PRCTL = 158,
    SET_TID_ADDRESS = 218,
    CLOCK_GETTIME = 228,
    OPENAT = 257,
    SET_ROBUST_LIST = 273,
    #[default]
    NONE = !0,
}

fn syscall<Endianness: ByteOrder>(cpu: &mut (impl CPU + MMU), kernel: &mut LinuxKernel<Endianness, X86>) {
    let rax = cpu.reg_read(RegisterX86::RAX.into()).unwrap();
    let mut debug_str = String::new();
    let syscall = SYSCALL::from(rax);
    let mut unknown_syscall = false;
    let rax = match SYSCALL::from(rax) {
        SYSCALL::READ => { do_syscall!(3, cpu, kernel::read, debug_str) }
        SYSCALL::WRITE => { do_syscall!(3, cpu, kernel::write, debug_str) }
        SYSCALL::CLOSE => { do_syscall!(1, cpu, kernel::close, debug_str) }
        SYSCALL::STAT => { do_syscall!(2, cpu, kernel::stat, debug_str) }
        SYSCALL::FSTAT => { do_syscall!(2, cpu, kernel::fstat, debug_str) }
        SYSCALL::MMAP => { do_syscall!(6, cpu, kernel::mmap, debug_str) }
        SYSCALL::MPROTECT => { do_syscall!(3, cpu, kernel::mprotect, debug_str) }
        SYSCALL::BRK => { do_syscall!(1, cpu, kernel::brk, debug_str) }
        SYSCALL::RT_SIGACTION => { do_syscall!(4, cpu, kernel::rt_sigaction, debug_str) }
        SYSCALL::RT_SIGPROCMASK => { do_syscall!(4, cpu, kernel::rt_sigprocmask, debug_str) }
        SYSCALL::PREAD64 => { do_syscall!(4, cpu, kernel::pread64, debug_str) }
        SYSCALL::WRITEV => { do_syscall!(3, cpu, kernel::writev, debug_str) }
        SYSCALL::ACCESS => { do_syscall!(2, cpu, kernel::access, debug_str) }
        SYSCALL::GETPID => { do_syscall!(0, cpu, kernel::getpid, debug_str) }
        SYSCALL::UNAME => { do_syscall!(1, cpu, kernel::uname, debug_str) }
        SYSCALL::GETTIMEOFDAY => { do_syscall!(2, cpu, kernel::gettimeofday, debug_str) }
        SYSCALL::ARCH_PRCTL => { do_syscall!(2, cpu, kernel::arch_prctl, debug_str) }
        SYSCALL::SET_TID_ADDRESS => { do_syscall!(1, cpu, kernel::set_tid_address, debug_str) }
        SYSCALL::CLOCK_GETTIME => { do_syscall!(2, cpu, kernel::clock_gettime, debug_str) }
        SYSCALL::OPENAT => { do_syscall!(4, cpu, kernel::openat, debug_str) }
        SYSCALL::SET_ROBUST_LIST => { do_syscall!(2, cpu, kernel::set_robust_list, debug_str) }
        SYSCALL::NONE => {
            debug_str = format!("(rax: {})", rax);
            unknown_syscall = true;
            !0
        }
    };
    if unknown_syscall {
        error!("SYSCALL: {:?} {} -> 0x{:X}", syscall, debug_str, rax);
        cpu.kill().unwrap();
    } else {
        trace!("SYSCALL: {:?} {} -> 0x{:X}", syscall, debug_str, rax);
    }
    cpu.reg_write(RegisterX86::RAX.into(), rax).unwrap();
}

impl<'a, CpuT, Endianness> LinuxX86<'a, CpuT, Endianness>
    where CpuT: CPU + MMU + Hooks<'a>, Endianness: ByteOrder + 'a {
    pub fn new(kernel: Rc<RefCell<LinuxKernel<Endianness, X86>>>, cpu: Rc<RefCell<CpuT>>, binary_path: PathBuf) -> Result<LinuxX86<'a, CpuT, Endianness>, CreationError> {
        let emu = LinuxX86 { binary_path, cpu, kernel: kernel.clone(), phantom: Default::default() };
        kernel.borrow_mut().add_stdio_handles();

        let mut cpu = emu.cpu.borrow_mut();
        kernel.borrow_mut().load_binary(&mut *cpu, emu.binary_path.as_path()).unwrap();

        let cb = move |cpu: &mut CpuT| {
            let mut kernel = kernel.borrow_mut();
            syscall(cpu, &mut *kernel);
        };
        cpu.add_syscall_hook(1, 0, Box::new(cb));
        drop(cpu);
        Ok(emu)
    }
}
