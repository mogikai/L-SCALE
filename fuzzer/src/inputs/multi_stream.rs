use libafl::inputs::Input;
use libafl::mutators::Mutator;
use std::path::Component;

/// A `MultiStreamInput` consists of several streams of data of the associated type `Stream: Input`
/// In most cases the type of `Stream` will be `BytesInput`.
/// Streams are associated with certain 'components' that group together different input streams.
/// The `MultiStreamInput` is generic over the type of the component identifier, allowing you to
/// use for example a string as ID, or if all components are known apriori, you could also use an
/// enum, which should be more efficient.
pub trait MultiStreamInput: Input {
    type Stream: Input;
    type ComponentId: Clone;

    fn component_streams(
        &self,
        component: &Self::ComponentId,
    ) -> impl Iterator<Item = &Self::Stream>;

    fn component_streams_mut(
        &mut self,
        component: &Self::ComponentId,
    ) -> impl Iterator<Item = &mut Self::Stream>;

    /// Sets the number of available components for this input to n.
    /// If there were previously more than n component streams, they are
    /// truncated to n. Otherwise, additional empty streams are created.
    fn set_available_streams(
        &mut self,
        component: &Self::ComponentId,
        n: usize,
    );

    fn components(&self) -> impl ExactSizeIterator<Item = &Self::ComponentId>;
    fn component(
        &self,
        index: usize,
    ) -> Option<Self::ComponentId>;
}
