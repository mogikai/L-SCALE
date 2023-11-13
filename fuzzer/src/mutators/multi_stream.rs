use std::marker::PhantomData;

use libafl::mutators::{
    MutationId,
    MutationResult,
    Mutator,
    MutatorsTuple,
};
use libafl::state::HasRand;
use libafl_bolts::rands::Rand;
use libafl_bolts::{
    Error,
    Named,
};

use crate::inputs::multi_stream::MultiStreamInput;

pub struct MultiStreamMutator<MutatorsTupleT, InputT, StateT>
where
    MutatorsTupleT: MutatorsTuple<InputT::Stream, StateT>,
    InputT: MultiStreamInput,
{
    name: String,
    mutators: MutatorsTupleT,
    _phantom: PhantomData<(InputT, StateT)>,
}

impl<MutatorsTupleT, InputT, StateT> MultiStreamMutator<MutatorsTupleT, InputT, StateT>
where
    MutatorsTupleT: MutatorsTuple<InputT::Stream, StateT>,
    StateT: HasRand,
    InputT: MultiStreamInput,
{
    pub fn new(mutators: MutatorsTupleT) -> Self {
        Self {
            name: "MultiStreamMutator".to_string(),
            mutators,
            _phantom: Default::default(),
        }
    }

    fn schedule(
        &self,
        state: &mut StateT,
    ) -> MutationId {
        debug_assert!(!self.mutators.is_empty());
        state.rand_mut().below(self.mutators.len() as u64).into()
    }
}

impl<MutatorsTupleT, InputT, StateT> Named for MultiStreamMutator<MutatorsTupleT, InputT, StateT>
where
    MutatorsTupleT: MutatorsTuple<InputT::Stream, StateT>,
    InputT: MultiStreamInput,
{
    fn name(&self) -> &str {
        todo!()
    }
}

impl<MutatorsTupleT, InputT, StateT> Mutator<InputT, StateT> for MultiStreamMutator<MutatorsTupleT, InputT, StateT>
where
    MutatorsTupleT: MutatorsTuple<InputT::Stream, StateT>,
    StateT: HasRand,
    InputT: MultiStreamInput,
{
    fn mutate(
        &mut self,
        state: &mut StateT,
        input: &mut InputT,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        let component = state.rand_mut().below(input.components().len() as u64);
        let component = input.component(component as usize).unwrap();
        let mut mutation_result = MutationResult::Skipped;
        for stream in input.component_streams_mut(&component) {
            let idx = self.schedule(state);
            self.mutators
                .get_and_mutate(idx, state, stream, stage_idx)?;
            mutation_result = MutationResult::Mutated;
        }

        Ok(mutation_result)
    }
}
