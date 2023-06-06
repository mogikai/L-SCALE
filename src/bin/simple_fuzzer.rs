use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use byteorder::LittleEndian;
use log::debug;

use fuzzyemu::emulator::abstractions::emulator::Emulator;
use fuzzyemu::emulator::abstractions::errors::GenericError;
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use fuzzyemu::emulator::platform::linux::architecture::x86::X86;
use fuzzyemu::emulator::platform::linux::emulator::LinuxX86Scheduler;
use fuzzyemu::emulator::platform::linux::kernel::LinuxKernelTask;
use fuzzyemu::emulator::system::clock::Clock;
use fuzzyemu::emulator::system::filesystem::Filesystem;
use fuzzyemu::emulator::system::hardware::unicorn::UCWrapper;
use fuzzyemu::emulator::system::memory::Memory;
use fuzzyemu::emulator::system::System;

fn main() -> Result<(), GenericError> {
    pretty_env_logger::init();
    let mut blocks = HashMap::<(u64, u64, u64), u64>::new();
    let mut prev_block: u64 = 0;
    let mut prev_block2: u64 = 0;

    {
        let cpu = UCWrapper::new();

        let filesystem = Filesystem::from_disk(1_073_741_824, Path::new("systems/ubuntu2204")).unwrap();
        let memory = Memory::new();
        let clock = Clock::new(Duration::from_nanos(1662647309301240322));
        let system = Rc::new(System::new(memory, filesystem, clock));
        // let memory = Rc::new(RefCell::new(Memory::new(
        //     &mut *cpu.borrow_mut(),
        //     0x0000_7fff_ffff_ffff,
        //     10 * 1024 * 1024, // 10 MiB
        //     0x0000_5000_0000_0000,
        //     0x0000_3000_0000_0000,
        // )));

        cpu.borrow_mut().add_block_hook(Box::new(|_, addr, _| {
            *blocks.entry((prev_block2, prev_block, addr)).or_insert(0) += 1;
            prev_block2 = prev_block;
            prev_block = addr;
            // TODO: Is this a performance bottleneck, do we have to use a simpler map?
        }));
        let vtlb_system = system.clone();
        cpu.borrow_mut()
            .add_vtlb_hook(Box::new(move |_uc, vaddr, _action| {
                vtlb_system.current_task.borrow().and_then(|task_id| {
                    vtlb_system
                        .memory
                        .borrow_mut()
                        .translate_vaddr(task_id, vaddr)
                })
            }))
            .unwrap();

        // TODO: Only one task for now. Need to implement a scheduler that switches tasks.
        let initial_task = LinuxKernelTask::<LittleEndian, X86>::new(system.clone(), 1337, 1);

        // let mut data = vec![b'A'; 0x01];
        // data.extend(vec![b'x', b'y', b'z', b'x', b'y', b'z', b'\n']);
        // let demo_handle = Box::new(ReadOnly::from(data));
        // kernel.borrow_mut().add_replacement_handle(0, demo_handle);
        let args = [];
        let env = ["SHELL=/bin/sh", "LD_DEBUG=all"]; //, "LD_BIND_NOW=true"];
                                                     // let eflags = cpu.borrow_mut().reg_read(RegisterX86::EFLAGS.into()).unwrap();
                                                     // cpu.borrow_mut().reg_write(RegisterX86::EFLAGS.into(), eflags | (1 << 22)).unwrap();
        let mut scheduler = LinuxX86Scheduler::new(
            system.clone(),
            initial_task,
            cpu,
            PathBuf::from("/home/user/open62541_server"),
            &args,
            &env,
        )
        .unwrap();
        scheduler.run(None, None).unwrap();
    }

    // debug!("Blocks: {:X?}", blocks);
    debug!("Unique 3-edges executed: {:?}", blocks.keys().fold(0, |acc, _| acc + 1));
    debug!(
        "Unique 2-edges executed: {:?}",
        blocks
            .keys()
            .map(|(_, b, c)| (*b, *c))
            .collect::<HashSet<(u64, u64)>>()
            .iter()
            .fold(0, |acc, _| acc + 1)
    );
    debug!(
        "Unique blocks executed: {:?}",
        blocks
            .keys()
            .map(|(_, _, c)| *c)
            .collect::<HashSet<u64>>()
            .iter()
            .fold(0, |acc, _| acc + 1)
    );
    debug!("Total 3-edges executed: {:?}", blocks.values().cloned().reduce(|acc, b| acc + b).unwrap());
    Ok(())
}
