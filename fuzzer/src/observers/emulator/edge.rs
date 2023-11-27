use std::cell::{
    Ref,
    RefCell,
};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;

use fuzzyemu::emulator::abstractions::cpu::address::EVAddr;
use fuzzyemu::emulator::abstractions::hooks::Hooks;

use crate::api::observer::{
    HasObservation,
    Observer,
};

#[derive(Debug, Default)]
struct InnerEdgeObserver {
    edge_counts: Vec<u64>,
    edge_map: BTreeMap<(EVAddr, EVAddr), usize>,
    /// The address for the block that was visited most recently.
    previous_address: Option<EVAddr>,
}

impl InnerEdgeObserver {
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
            self.edge_counts[id] += 1;
        }
        self.previous_address = Some(address);
    }

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
        self.edge_counts.iter_mut().for_each(|count| *count = 0);
    }
}

#[derive(Debug, Default)]
pub struct EdgeObserver {
    inner: Rc<RefCell<InnerEdgeObserver>>,
}

impl EdgeObserver {
    pub fn new<CpuT: Hooks>(cpu: &Rc<RefCell<CpuT>>) -> Self {
        let observer = Self {
            inner: Default::default(),
        };
        let hook_inner = observer.inner.clone();
        cpu.borrow_mut()
            .add_block_hook(Box::new(move |_cpu, address, _instruction_count| {
                hook_inner.borrow_mut().visit_block(address);
            }))
            .unwrap();
        observer
    }
}

impl Observer for EdgeObserver {
    fn gather(&mut self) {}

    fn clear(&mut self) {
        self.inner.borrow_mut().reset();
    }
}

impl HasObservation for EdgeObserver {
    type Observation = Vec<u64>;

    fn observation(&self) -> impl Deref<Target = Self::Observation> {
        Ref::map(self.inner.borrow(), |inner| &inner.edge_counts)
    }
}
