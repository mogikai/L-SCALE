mod graph;

use slotmap::{
    SecondaryMap,
    SlotMap,
    new_key_type,
};

pub trait Observable {
    /// Observe the observable in the state specified by prefix when applying the provided word.
    fn observe(
        &mut self,
        prefix: StateId,
        word: Word,
    ) -> Observation;
}

new_key_type! {pub struct WordId;}
new_key_type! {pub struct ObservationsId;}
new_key_type! {pub struct StateId;}
new_key_type! {pub struct SuffixId;}

pub type Word = String;

#[derive(Debug, Default)]
pub enum Observation {
    #[default]
    Missing,
    Observed(String),
}

pub type Row<K, V> = SecondaryMap<K, V>;
pub type Column<K, V> = SecondaryMap<K, V>;

/**
We have states and candidates. A candidate is simply a potential state that could possibly be reached by applying
another word to an existing state, i.e., extending the prefix by a single word.
*/

#[derive(Debug, Default)]
struct StateObservations {
    observations: SecondaryMap<SuffixId, Observation>,
}

#[derive(Debug, Default)]
struct State {
    /// The state from which this state was derived by applying the word `prefix_extension`.
    prefix_state: StateId,
    /// The word that was applied to the state `prefix_state` to derive this state.
    prefix_extension: WordId,
    observations: ObservationsId,
}

#[derive(Debug)]
struct ObservationTable {
    /// The list of all observation rows. Each state or state candidate is associated with one row.
    observations: SlotMap<ObservationsId, StateObservations>,
    /// The states table contains those prefixes for which we can be sure that they are a state.
    states: SlotMap<StateId, State>,
    final_states: SecondaryMap<StateId, ObservationsId>,
    candidate_states: SecondaryMap<StateId, ObservationsId>,
}

impl Default for ObservationTable {
    fn default() -> Self {
        let mut slf = Self {
            observations: Default::default(),
            states: Default::default(),
            final_states: Default::default(),
            candidate_states: Default::default(),
        };
        let initial_observations = slf.observations.insert(StateObservations::default());
        // Insert the initial state which refers to itself.
        slf.states.insert_with_key(|state_id| State {
            prefix_state: state_id,
            // TODO: How to handle the empty word? Do we need to handle the initial state differently?
            prefix_extension: WordId::default(),
            observations: initial_observations,
        });
        slf
    }
}

impl ObservationTable {
    pub fn debug_observations(&self) {
        for (_, row) in self.observations.iter() {
            println!("{:?}", row.observations);
        }
    }
}

#[derive(Debug, Default)]
struct Learner {
    /// A map of words that is used to facilitate easy comparison of words.
    /// If a new word needs to be added to the learner it will be looked up here.
    /// If it already exists, then the existing word id is returned. Otherwise a new one is created.
    words: SlotMap<WordId, Word>,

    observation_table: ObservationTable,
}

#[cfg(test)]
mod test {
    use crate::Learner;

    #[test]
    fn test() {
        let learner = Learner::default();
        learner.observation_table.debug_observations();
    }
}
