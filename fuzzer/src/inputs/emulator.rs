use libafl::inputs::{
    BytesInput,
    Input,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::inputs::multi_stream::MultiStreamInput;

#[derive(Debug, Clone)]
pub enum EmulatorInputComponent {
    Filesystem,
    Tcp,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct EmulatorInput {
    // The SocketID might differ between runs, since the id depends on how many snapshots and restores were
    // performed already. The Order of the socket ids however should always be the same. Hence, we only
    // keep a vector of inputs for each stream and they will be assigned to open streams in order.
    tcp_streams: Vec<BytesInput>,
}

const COMPONENT_TYPES: [EmulatorInputComponent; 1] = [EmulatorInputComponent::Tcp];

impl EmulatorInput {
    pub fn new() -> Self {
        Self {
            tcp_streams: Default::default(),
        }
    }
}

impl Input for EmulatorInput {
    fn generate_name(
        &self,
        idx: usize,
    ) -> String {
        todo!()
    }
}

impl MultiStreamInput for EmulatorInput {
    type Stream = BytesInput;
    type ComponentId = EmulatorInputComponent;

    fn component_streams(
        &self,
        component: &EmulatorInputComponent,
    ) -> impl Iterator<Item = &Self::Stream> {
        match component {
            EmulatorInputComponent::Tcp => self.tcp_streams.iter(),
            _ => todo!(),
        }
    }

    fn component_streams_mut(
        &mut self,
        component: &Self::ComponentId,
    ) -> impl Iterator<Item = &mut Self::Stream> {
        match component {
            EmulatorInputComponent::Tcp => self.tcp_streams.iter_mut(),
            _ => todo!(),
        }
    }

    fn components(&self) -> impl ExactSizeIterator<Item = &Self::ComponentId> {
        COMPONENT_TYPES.iter()
    }

    fn component(
        &self,
        index: usize,
    ) -> Option<Self::ComponentId> {
        COMPONENT_TYPES.get(index).cloned()
    }
}
