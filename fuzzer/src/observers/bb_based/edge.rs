use std::cell::RefCell;
use std::cmp::max;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::Deref;
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

use core_affinity::CoreId;
use parking_lot::{
    Mutex,
    MutexGuard,
};

use mogi_emulator::emulator::abstractions::hooks::Hooks;
use mogi_utilities::addresses::EVAddr;

use crate::api::observer::{
    DebuggableObserve,
    HasObservation,
    Observe,
    Observer,
};

#[derive(Debug, Default)]
struct SharedEdgeObserver {
    edge_counts: Vec<u64>,
    edge_map: BTreeMap<(EVAddr, EVAddr), usize>,
    /// The address for the block that was visited most recently.
    previous_address: Option<EVAddr>,
    // The highest observed index. The returned map will be truncated to this index, since every entry
    // afterwards will be zero.
    highest_observed_index: usize,
}

impl SharedEdgeObserver {
    fn edge_index(
        &mut self,
        edge: (EVAddr, EVAddr),
    ) -> usize {
        *self.edge_map.entry(edge).or_insert_with(|| {
            let id = self.edge_counts.len();
            self.edge_counts.push(0);
            id
        })
    }

    /// Resets the observer data. This sets the counts of all edges to zero.
    /// The mapping for edges to indices is kept, such that different runs
    /// can be compared, since they have the same mapping.
    fn reset(&mut self) {
        self.previous_address = None;
        self.highest_observed_index = 0;
        self.edge_counts.iter_mut().for_each(|count| *count = 0);
    }
}

impl Observe for SharedEdgeObserver {
    type RawObservation = EVAddr;

    /// Logs the address and increments the visit count for the edge (prev_address, address).
    ///
    /// # Arguments
    ///
    /// * `address: EVAddr` - The address of the block to visit.
    fn observe(
        &mut self,
        address: &Self::RawObservation,
    ) {
        // The previous address does not exist if we are starting from the beginning or from a specific state.
        if let Some(previous) = self.previous_address {
            let edge = (previous, *address);
            let id = self.edge_index(edge);
            self.highest_observed_index = max(self.highest_observed_index, id);
            self.edge_counts[id] = self.edge_counts[id].saturating_add(1);
        }
        self.previous_address = Some(*address);
    }
}

#[derive(Debug, Default)]
pub struct EdgeHitCountObserver {
    shared_observer: Arc<Mutex<SharedEdgeObserver>>,
}

impl EdgeHitCountObserver {
    pub fn new() -> Self {
        Self {
            shared_observer: Default::default(),
        }
    }

    pub fn shared_observe(&self) -> Arc<Mutex<dyn DebuggableObserve<RawObservation = EVAddr>>> {
        self.shared_observer.clone()
    }
}

impl Observer for EdgeHitCountObserver {
    fn gather(&mut self) {}

    fn clear(&mut self) {
        self.shared_observer.lock().reset();
    }
}

impl HasObservation for EdgeHitCountObserver {
    type Observation = [u64];

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        MutexGuard::map(self.shared_observer.lock(), |inner| {
            if inner.edge_counts.is_empty() {
                &mut [] as &mut [u64]
            } else {
                &mut inner.edge_counts[..=inner.highest_observed_index]
            }
        })
    }
}

#[derive(Debug, Default)]
struct InnerEdgeHitObserver {
    // TODO: Optimize and make bitmap out of this
    edge_hits: Vec<bool>,
    edge_map: BTreeMap<(EVAddr, EVAddr), usize>,
    /// The address for the block that was visited most recently.
    previous_address: Option<EVAddr>,
    // The highest observed index. The returned map will be truncated to this index, since every entry
    // afterwards will be zero.
    highest_observed_index: usize,
}

impl InnerEdgeHitObserver {
    /// Logs the address and increments the visit count for the edge (prev_address, address).
    ///
    /// # Arguments
    ///
    /// * `address: EVAddr` - The address of the block to visit.
    fn visit_block(
        &mut self,
        address: EVAddr,
    ) {
        // The previous address does not exist if we are starting from the beginning or from a specific state.
        if let Some(previous) = self.previous_address {
            let edge = (previous, address);
            let id = self.edge_index(edge);
            self.highest_observed_index = max(self.highest_observed_index, id);
            self.edge_hits[id] = true;
        }
        self.previous_address = Some(address);
    }

    fn edge_index(
        &mut self,
        edge: (EVAddr, EVAddr),
    ) -> usize {
        *self.edge_map.entry(edge).or_insert_with(|| {
            let id = self.edge_hits.len();
            self.edge_hits.push(false);
            id
        })
    }

    /// Resets the observer data. This sets the counts of all edges to zero.
    /// The mapping for edges to indices is kept, such that different runs
    /// can be compared, since they have the same mapping.
    fn reset(&mut self) {
        self.previous_address = None;
        self.highest_observed_index = 0;
        self.edge_hits.iter_mut().for_each(|hit| *hit = false);
    }
}

#[derive(Debug, Default)]
pub struct EdgeHitObserver {
    inner: Arc<Mutex<InnerEdgeHitObserver>>,
    remaining_edges: Arc<AtomicUsize>,
}

impl EdgeHitObserver {
    pub fn new<CpuT: Hooks>(cpu: &Rc<RefCell<CpuT>>) -> Self {
        let observer = Self {
            inner: Default::default(),
            remaining_edges: Default::default(),
        };
        let inner = observer.inner.clone();
        let remaining_edges = observer.remaining_edges.clone();
        let (sender, receiver) = mpsc::channel::<EVAddr>();
        thread::spawn(move || {
            core_affinity::set_for_current(CoreId { id: 1 });
            for block_addr in receiver {
                remaining_edges.fetch_sub(1, Ordering::Release);
                inner.lock().visit_block(block_addr);
            }
        });
        let remaining_edges = observer.remaining_edges.clone();
        cpu.borrow_mut()
            .add_block_hook(
                EVAddr::from(1u64),
                EVAddr::from(0u64),
                Box::new(move |_cpu, address, _instruction_count| {
                    remaining_edges.fetch_add(1, Ordering::Acquire);
                    sender.send(address).unwrap();
                }),
            )
            .unwrap();
        observer
    }
}

impl Observer for EdgeHitObserver {
    fn gather(&mut self) {
        while self.remaining_edges.load(Ordering::Relaxed) != 0 {}
    }

    fn clear(&mut self) {
        self.inner.lock().reset();
    }
}

impl HasObservation for EdgeHitObserver {
    type Observation = [bool];

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        MutexGuard::map(self.inner.lock(), |inner| &mut inner.edge_hits[..=inner.highest_observed_index])
    }
}
