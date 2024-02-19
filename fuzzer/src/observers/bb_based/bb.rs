use arrayvec::ArrayVec;
use core_affinity::CoreId;
use fuzzyemu::emulator::abstractions::cpu::address::{
    EVAddr,
    EVA_NULL,
};
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
use std::mem::take;
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
    DebuggableObserve,
    HasObservation,
    Observe,
    Observer,
};

#[derive(Debug, Default)]
pub struct BlockHitObserver {
    shared_observer: Arc<Mutex<SharedBlockHitObserver>>,
}

#[derive(Debug, Default)]
struct SharedBlockHitObserver {
    basic_blocks_hit: BTreeSet<EVAddr>,
}

impl BlockHitObserver {
    pub fn new() -> Self {
        Self {
            shared_observer: Default::default(),
        }
    }

    pub fn shared_observe(&self) -> Arc<Mutex<dyn DebuggableObserve<RawObservation = EVAddr>>> {
        self.shared_observer.clone()
    }
}

impl Observer for BlockHitObserver {
    fn gather(&mut self) {}

    fn clear(&mut self) {
        self.shared_observer.lock().basic_blocks_hit.clear();
    }
}

impl Observe for SharedBlockHitObserver {
    type RawObservation = EVAddr;

    fn observe(
        &mut self,
        observation: &Self::RawObservation,
    ) {
        self.basic_blocks_hit.insert(*observation);
    }
}

impl HasObservation for BlockHitObserver {
    type Observation = BTreeSet<EVAddr>;

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        MutexGuard::map(self.shared_observer.lock(), |inner| &mut inner.basic_blocks_hit)
    }
}
