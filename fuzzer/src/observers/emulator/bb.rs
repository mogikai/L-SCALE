use core_affinity::CoreId;
use fuzzyemu::emulator::abstractions::cpu::address::EVAddr;
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use parking_lot::{
    Mutex,
    MutexGuard,
};
use std::cell::{
    Ref,
    RefCell,
};
use std::cmp::max;
use std::collections::{
    BTreeMap,
    BTreeSet,
    HashSet,
};
use std::fmt::Debug;
use std::ops::{
    Add,
    Deref,
};
use std::rc::Rc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};
use std::sync::{
    mpsc,
    Arc,
};
use std::thread;

use crate::api::observer::{
    HasObservation,
    Observer,
};

#[derive(Debug, Default)]
struct InnerBlockObserver {
    basic_blocks_hit: BTreeSet<EVAddr>,
}

impl InnerBlockObserver {
    /// Logs the address and increments the visit count for the edge (prev_address, address).
    ///
    /// # Arguments
    ///
    /// * `address: EVAddr` - The address of the block to visit.
    fn visit_block(
        &mut self,
        address: EVAddr,
    ) {
        self.basic_blocks_hit.insert(address);
    }
    /// Resets the observer data. This sets the counts of all edges to zero.
    /// The mapping for edges to indices is kept, such that different runs
    /// can be compared, since they have the same mapping.
    fn reset(&mut self) {
        self.basic_blocks_hit.clear();
    }
}

#[derive(Debug, Default)]
pub struct BlockHitObserver {
    inner: Arc<Mutex<InnerBlockObserver>>,
    remaining_edges: Arc<AtomicUsize>,
}

impl BlockHitObserver {
    pub fn new<CpuT: Hooks>(cpu: &Rc<RefCell<CpuT>>) -> Self {
        let observer = Self {
            inner: Default::default(),
            remaining_edges: Default::default(),
        };
        let inner = observer.inner.clone();
        let remaining_edges = observer.remaining_edges.clone();
        let (sender, receiver) = mpsc::channel::<EVAddr>();
        thread::spawn(move || {
            core_affinity::set_for_current(CoreId { id: 3 });
            for block_addr in receiver {
                remaining_edges.fetch_sub(1, Ordering::Release);
                inner.lock().visit_block(block_addr);
            }
        });
        let remaining_edges = observer.remaining_edges.clone();
        cpu.borrow_mut()
            .add_block_hook(Box::new(move |_cpu, address, _instruction_count| {
                remaining_edges.fetch_add(1, Ordering::Acquire);
                sender.send(address).unwrap();
                // hook_inner.borrow_mut().visit_block(address);
            }))
            .unwrap();
        observer
    }
}

impl Observer for BlockHitObserver {
    fn gather(&mut self) {
        while self.remaining_edges.load(Ordering::Relaxed) != 0 {}
    }

    fn clear(&mut self) {
        self.inner.lock().reset();
    }
}

impl HasObservation for BlockHitObserver {
    type Observation = BTreeSet<EVAddr>;

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        MutexGuard::map(self.inner.lock(), |inner| &mut inner.basic_blocks_hit)
    }
}
