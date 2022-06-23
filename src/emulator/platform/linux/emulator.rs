use std::{fs, io};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

use byteorder::ByteOrder;
use linux_libc_auxv::AuxVarType;
use num_enum::FromPrimitive;

use crate::emulator::abstractions::cpu::{CPU, CPUError, Permission, RegisterX86, Vaddr};
use crate::emulator::abstractions::emulator::{Emulator, StopReason};
use crate::emulator::platform::linux::kernel::LinuxKernel;
use crate::emulator::system::filesystem::Filesystem;
use crate::emulator::system::memory;
use crate::emulator::system::memory::{Memory, MemoryWrite};

#[derive(Debug)]
pub enum CreationError {
    ProgramHeaderErr,
    Unknown,
}

pub struct LinuxX86<'a, CpuT, Endianness> where CpuT: CPU<'a>, Endianness: ByteOrder {
    binary_path: PathBuf,
    cpu: Rc<RefCell<CpuT>>,
    kernel: Rc<RefCell<LinuxKernel<Endianness>>>,
    phantom: PhantomData<&'a CpuT>,
}

impl From<CPUError> for StopReason {
    fn from(_: CPUError) -> Self {
        StopReason::Unknown
    }
}

impl<'a, CpuT, Endianness> Emulator for LinuxX86<'a, CpuT, Endianness> where CpuT: CPU<'a>, Endianness: ByteOrder {
    fn run(&mut self) -> Result<(), StopReason> {
        self.cpu.borrow_mut().emulate(None, None, None)?;
        Ok(())
    }
}

macro_rules! do_syscall {
	(0, $uc:ident, $name:ident::$fun:ident) => {
		{
			trace!("() ");
			$name.$fun($uc)
		}
	};
	(1, $uc:ident, $name:ident::$fun:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			trace!("(0x{:X}) ", rdi);
			$name.$fun($uc, rdi)
		}
	};
	(2, $uc:ident, $name:ident::$fun:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			trace!("(0x{:X}, 0x{:X}) ", rdi, rsi);
			$name.$fun($uc, rdi, rsi)
		}
	};
	(3, $uc:ident, $name:ident::$fun:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			trace!("(0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx);
			$name.$fun($uc, rdi, rsi, rdx)
		}
	};
	(4, $uc:ident, $name:ident::$fun:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
			trace!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10);
			$name.$fun($uc, rdi, rsi, rdx, r10)
		}
	};
	(6, $uc:ident, $name:ident::$fun:ident) => {
		{
			let rdi = $uc.reg_read(RegisterX86::RDI.into()).unwrap();
			let rsi = $uc.reg_read(RegisterX86::RSI.into()).unwrap();
			let rdx = $uc.reg_read(RegisterX86::RDX.into()).unwrap();
			let r10 = $uc.reg_read(RegisterX86::R10.into()).unwrap();
			let r8 = $uc.reg_read(RegisterX86::R8.into()).unwrap();
			let r9 = $uc.reg_read(RegisterX86::R9.into()).unwrap();
			trace!("(0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}, 0x{:X}) ", rdi, rsi, rdx, r10, r8, r9);
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
    PREAD64 = 17,
    WRITEV = 20,
    ACCESS = 21,
    GETPID = 39,
    UNAME = 63,
    ARCH_PRCTL = 158,
    OPENAT = 257,
    #[default]
    NONE = !0,
}

fn syscall<'a, CpuT: CPU<'a>, Endianness: ByteOrder>(cpu: &mut CpuT, kernel: &mut LinuxKernel<Endianness>) {
    let rax = cpu.reg_read(RegisterX86::RAX.into()).unwrap();
    trace!("SYSCALL: {:?}", SYSCALL::from(rax));
    let rax = match SYSCALL::from(rax) {
        SYSCALL::READ => { do_syscall!(3, cpu, kernel::read) }
        SYSCALL::WRITE => { do_syscall!(3, cpu, kernel::write) }
        SYSCALL::CLOSE => { do_syscall!(1, cpu, kernel::close) }
        SYSCALL::STAT => { do_syscall!(2, cpu, kernel::stat) }
        SYSCALL::FSTAT => { do_syscall!(2, cpu, kernel::fstat) }
        SYSCALL::MMAP => { do_syscall!(6, cpu, kernel::mmap) }
        SYSCALL::MPROTECT => { do_syscall!(3, cpu, kernel::mprotect) }
        SYSCALL::BRK => { do_syscall!(1, cpu, kernel::brk) }
        SYSCALL::PREAD64 => { do_syscall!(4, cpu, kernel::pread64) }
        SYSCALL::WRITEV => { do_syscall!(3, cpu, kernel::writev) }
        SYSCALL::ACCESS => { do_syscall!(2, cpu, kernel::access) }
        SYSCALL::GETPID => { do_syscall!(0, cpu, kernel::getpid) }
        SYSCALL::UNAME => { do_syscall!(1, cpu, kernel::uname) }
        SYSCALL::ARCH_PRCTL => { do_syscall!(2, cpu, kernel::arch_prctl) }
        SYSCALL::OPENAT => { do_syscall!(4, cpu, kernel::openat) }
        SYSCALL::NONE => {
            trace!("unknown(rax: {})", rax);
            !0
        }
    };
    trace!("-> 0x{:X}", rax);
    cpu.reg_write(RegisterX86::RAX.into(), rax).unwrap();
}

fn getpath(dir: &Path, file: &Path) -> Result<PathBuf, io::Error> {
    let p = if file.is_absolute() {
        file.strip_prefix("/").unwrap()
    } else {
        file
    };
    let dir = dir.canonicalize()?;
    let p = dir.join(p).canonicalize()?;
    if p.starts_with(dir) { Ok(p) } else { Err(io::ErrorKind::NotFound.into()) }
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
    return perms;
}

impl<'a, CpuT, Endianness> LinuxX86<'a, CpuT, Endianness> where CpuT: CPU<'a>, Endianness: ByteOrder + 'a {
    pub fn new(filesystem: Filesystem, memory: Memory<Endianness>, cpu: Rc<RefCell<CpuT>>, binary_path: PathBuf) -> Result<LinuxX86<'a, CpuT, Endianness>, CreationError> {
        let kernel = Rc::new(RefCell::new(LinuxKernel::new(memory, filesystem)));
        let mut emu = LinuxX86 { binary_path, cpu, kernel: kernel.clone(), phantom: Default::default() };
        {
            let mut cpu = emu.cpu.borrow_mut();
            cpu.reg_write(RegisterX86::RSP.into(), 0x4000000000000000).expect("failed to write RSP");
            let cb = move |cpu: &mut CpuT| {
                let mut kernel = kernel.borrow_mut();
                syscall(cpu, &mut *kernel);
            };
            cpu.add_syscall_hook(1, 0, Box::new(cb));
        }
        let (interpreter_path, entrypoint) = emu.load_binary()?;
        emu.initstack(&interpreter_path, entrypoint)?;

        Ok(emu)
    }
    fn load_binary(&mut self) -> Result<(PathBuf, Vaddr), CreationError> {
        let mut cpu = self.cpu.borrow_mut();
        let mut kernel = self.kernel.borrow_mut();
        let root_dir = kernel.filesystem().root();
        let buffer = fs::read(getpath(root_dir, &self.binary_path).unwrap()).unwrap();
        let elf = goblin::elf::Elf::parse(&buffer).unwrap();
        let interpreter_path = getpath(root_dir, Path::new(elf.interpreter.unwrap())).unwrap();
        let buffer = fs::read(&interpreter_path).unwrap();
        let interpreter = goblin::elf::Elf::parse(&buffer).unwrap();
        cpu.reg_write(RegisterX86::RIP.into(), interpreter.entry).unwrap();
        //TODO iterator
        for ph in interpreter.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD {
                let perms = getperms(&ph);
                if ph.p_offset + ph.p_filesz > buffer.len().try_into().unwrap() {
                    return Err(CreationError::ProgramHeaderErr);
                }
                let data: &[u8] = &buffer[ph.file_range()];
                let vm_range = ph.p_vaddr..(ph.p_vaddr + ph.p_memsz);
                info!("vm_range   0x{:X}..0x{:X}", vm_range.start, vm_range.end);
                info!("file_range 0x{:X}..0x{:X}", ph.file_range().start, ph.file_range().end);
                kernel.memory_mut().map_data(data, vm_range, perms, &mut *cpu).unwrap();
            }
        }
        kernel.memory_mut().map_data(&[0; 4096 * 4], (0x4000000000000000 - (4096 * 4))..0x4000000000000000, Permission::READ | Permission::WRITE, &mut *cpu).unwrap();
        Ok((interpreter_path, interpreter.entry))
    }

    fn initstack(&mut self, interpreter_path: &Path, entrypoint: Vaddr) -> Result<(), CreationError> {
        info!("Initializing stack");
        let mut kernel = self.kernel.borrow_mut();
        let mem = kernel.memory_mut();
        let mut cpu = self.cpu.borrow_mut();
        let mut env: Vec<u64> = Vec::new();
        let mut argv: Vec<u64> = Vec::new();
        let mut aux: Vec<(u64, u64)> = Vec::new();
        crate::push_stack!(mem, 0u64, RegisterX86::RSP.into(), cpu);
        env.push(crate::push_stack!(mem, "SHELL=/bin/sh", RegisterX86::RSP.into(), cpu));
        env.push(crate::push_stack!(mem, "LD_DEBUG=all", RegisterX86::RSP.into(), cpu));
        env.push(0);
        argv.push(2);
        argv.push(crate::push_stack!(mem, interpreter_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
        argv.push(crate::push_stack!(mem, self.binary_path.to_str().unwrap(), RegisterX86::RSP.into(), cpu));
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
}