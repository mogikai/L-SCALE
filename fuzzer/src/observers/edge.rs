use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::rc::Rc;

use fuzzyemu::emulator::abstractions::cpu::address::EVAddr;
use fuzzyemu::emulator::abstractions::hooks::Hooks;
use libafl::inputs::UsesInput;
use libafl::observers::Observer;
use libafl_bolts::Named;
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};

#[derive(Debug, Default, Serialize, Deserialize)]
struct InnerEdgeObserver {
    edge_counts: Vec<u64>,
    edge_map: BTreeMap<(EVAddr, EVAddr), usize>,
    last_address: EVAddr,
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
        let edge = (self.last_address, address);
        let id = self.edge_index(edge);
        self.edge_counts[id] += 1;
        self.last_address = address;
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
}

#[derive(Debug, Default)]
pub struct EdgeObserver {
    inner: Rc<RefCell<InnerEdgeObserver>>,
}

impl Serialize for EdgeObserver {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

impl<'de> Deserialize<'de> for EdgeObserver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
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

impl Named for EdgeObserver {
    fn name(&self) -> &str {
        "EdgeObserver"
    }
}

impl<StateT: UsesInput> Observer<StateT> for EdgeObserver {}
