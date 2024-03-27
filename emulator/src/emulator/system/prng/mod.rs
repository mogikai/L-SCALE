use std::collections::BTreeMap;
use std::fmt;

use mogi_system::errors::generic::{
    GenericError,
    GenericErrorKind,
};
use rand::{
    Rng,
    RngCore,
    SeedableRng,
};

use crate::emulator::abstractions::restore::Restorable;

#[derive(Debug, Default, Clone)]
pub struct SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng + Clone,
    TaskId: Ord + Copy + Clone + Default,
{
    master_prng: Prng, // readonly master PRNG from which others are cloned from
    prngs: BTreeMap<TaskId, Prng>,

    snapshots: Vec<BTreeMap<TaskId, Prng>>,
}

impl<Prng, TaskId> SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng + Clone,
    TaskId: Ord + Copy + Clone + Default,
{
    pub fn new(seed: Prng::Seed) -> Self {
        Self {
            master_prng: Prng::from_seed(seed),
            prngs: BTreeMap::new(),
            snapshots: Vec::new(),
        }
    }

    pub fn get_random_bytes(
        &mut self,
        task_id: TaskId,
        n: usize,
    ) -> Vec<u8> {
        let mut vec = vec![0u8; n];
        if let Some(prng) = self.prngs.get_mut(&task_id) {
            prng.fill_bytes(&mut vec[..]);
        } else {
            let mut new_prng = self.master_prng.clone();
            new_prng.fill_bytes(&mut vec[..]);
            self.prngs.insert(task_id, new_prng);
        }

        vec
    }
}

impl<Prng, TaskId> Restorable for SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng + Clone,
    TaskId: Ord + Copy + Clone + Default,
{
    fn take_snapshot(&mut self) -> Result<(), mogi_system::errors::generic::GenericError> {
        self.snapshots.push(self.prngs.clone());
        Ok(())
    }

    fn discard_current_and_restore_previous_snapshot(
        &mut self
    ) -> Result<(), mogi_system::errors::generic::GenericError> {
        if self.snapshots.len() <= 1 {
            Err(GenericError::new(GenericErrorKind::Unknown, "no previous snapshot available"))?
        }

        self.snapshots.pop();
        self.restore_snapshot()
    }

    fn restore_snapshot(&mut self) -> Result<(), mogi_system::errors::generic::GenericError> {
        self.prngs.clone_from(
            self.snapshots
                .last()
                .ok_or(GenericError::new(GenericErrorKind::Unknown, "no snapshot to restore"))?,
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct Seed<R>
where
    R: SeedableRng,
{
    seed: <R as SeedableRng>::Seed,
}

impl<R> fmt::Debug for Seed<R>
where
    R: SeedableRng,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        todo!()
    }
}
