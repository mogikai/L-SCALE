//! A collector that collects information about basic blocks being hit during execution.
//! The collector allows other observers to hook into this collector to be notified about
//! any collected blocks.
//!
use std::cell::RefCell;
use std::ops::{
    Range,
    RangeBounds,
};
use std::rc::Rc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};
use std::sync::Arc;
use std::thread;

use core_affinity::CoreId;
use crossbeam::channel::{
    Receiver,
    Sender,
};
use parking_lot::{
    Condvar,
    Mutex,
    RwLock,
};
use tracing::{
    debug,
    trace,
};

use mogi_emulator::emulator::abstractions::hooks::Hooks;
use mogi_emulator::emulator::interface::meter::Meter;
use mogi_utilities::addresses::EVAddr;

use crate::api::observer::{
    DebuggableObserve,
    DebuggableObserver,
};

#[derive(Debug)]
pub struct BasicBlockCollector {
    shared_data: Arc<BasicBlockCollectorSharedData>,
    internal_collector: Rc<RefCell<InternalCollector>>,
}

#[derive(Debug)]
struct BasicBlockCollectorSharedData {
    observers: Mutex<Vec<Arc<Mutex<dyn DebuggableObserve<RawObservation = EVAddr>>>>>,
    remaining_blocks: AtomicUsize,
    block_receiver: Mutex<Option<Receiver<EVAddr>>>,
    collecting: Mutex<bool>,
    collecting_cvar: Condvar,
}

#[derive(Debug)]
struct InternalCollector {
    sender: Option<Sender<EVAddr>>,
    shared_data: Arc<BasicBlockCollectorSharedData>,
}

impl InternalCollector {
    pub fn new(shared_data: Arc<BasicBlockCollectorSharedData>) -> Self {
        Self {
            sender: Default::default(),
            shared_data,
        }
    }

    fn collect(
        &mut self,
        address: EVAddr,
    ) {
        self.shared_data
            .remaining_blocks
            .fetch_add(1, Ordering::Acquire);
        self.sender
            .as_mut()
            .expect("Missing start call to collector")
            .send(address)
            .unwrap();
    }
}

impl BasicBlockCollector {
    pub fn new<'a, T: Iterator<Item = &'a Arc<Mutex<dyn DebuggableObserve<RawObservation = EVAddr>>>>, CpuT: Hooks>(
        cpu: &Rc<RefCell<CpuT>>,
        start: EVAddr,
        end: EVAddr,
        observers: T,
    ) -> Self {
        let shared_data = Arc::new(BasicBlockCollectorSharedData {
            observers: Mutex::new(observers.cloned().collect()),
            remaining_blocks: Default::default(),
            block_receiver: Default::default(),
            collecting: Default::default(),
            collecting_cvar: Default::default(),
        });
        let collector = BasicBlockCollector {
            shared_data: shared_data.clone(),
            internal_collector: Rc::new(RefCell::new(InternalCollector::new(shared_data.clone()))),
        };
        thread::spawn(move || {
            // TODO: Choose the core id dynamic or make it configurable to allow multiple fuzzing instances
            core_affinity::set_for_current(CoreId { id: 1 });
            loop {
                let mut collecting = shared_data.collecting.lock();
                if !*collecting {
                    shared_data.collecting_cvar.wait(&mut collecting);
                }
                let observers = shared_data.observers.lock();
                let mut locked_observers = observers.iter().map(|o| o.lock()).collect::<Vec<_>>();
                let receiver = shared_data.block_receiver.lock().take().unwrap();
                for block_addr in receiver {
                    shared_data.remaining_blocks.fetch_sub(1, Ordering::Release);
                    for observer in locked_observers.iter_mut() {
                        observer.observe(&block_addr)
                    }
                }
                *collecting = false;
                shared_data.collecting_cvar.notify_one();
            }
        });
        let internal_collector = collector.internal_collector.clone();
        cpu.borrow_mut()
            .add_block_hook(
                start,
                end,
                Box::new(move |_cpu, address, _instruction_count| internal_collector.borrow_mut().collect(address)),
            )
            .unwrap();
        collector
    }
}

impl Meter for BasicBlockCollector {
    fn start(&mut self) {
        trace!("Starting basic block collector");
        // TODO: It might make more sense to re-use the channel and signal completion in a different way.
        //       For short emulation runs this seems to take up a significant portion of the runtime.
        let (sender, receiver) = crossbeam::channel::bounded::<EVAddr>(1_000_000);
        let _ = self.shared_data.block_receiver.lock().insert(receiver);
        let _ = self.internal_collector.borrow_mut().sender.insert(sender);
        // Start the collection thread back up
        let mut collecting = self.shared_data.collecting.lock();
        *collecting = true;
        self.shared_data.collecting_cvar.notify_one();
    }

    fn stop(&mut self) {
        trace!("Stopping basic block collector");
        // Close sender.
        self.internal_collector.borrow_mut().sender.take();
        // Wait for collecting to be set to false, i.e. until the collection has finished.
        let mut collecting = self.shared_data.collecting.lock();
        if *collecting {
            self.shared_data.collecting_cvar.wait(&mut collecting);
        }
    }
}
