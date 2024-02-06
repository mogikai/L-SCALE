use std::{fmt, io::Read};
use std::collections::BTreeMap;

use rand::{Rng, RngCore, SeedableRng};

use crate::emulator::{abstractions::{restore::Restorable, system::SystemControl}, platform::linux::kernel::task_id::TaskId};

#[derive(Debug, Default, Clone)]
pub struct SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng + Clone,
    TaskId: Ord + Copy + Clone + Default
{
    master_prng: Prng,
    prngs: BTreeMap<TaskId, Prng>
}

impl<Prng, TaskId> SeedablePrngMap<Prng, TaskId>
where
    Prng: Rng + SeedableRng + Clone,
    TaskId: Ord + Copy + Clone + Default
{
    pub fn new(seed: Prng::Seed) -> Self {
        Self {
            master_prng: Prng::from_seed(seed),
            prngs: BTreeMap::new()
        }
    }

    pub fn get_random_bytes(&mut self, task_id: TaskId, n: usize) -> Vec<u8> {
        let mut vec = vec![0u8; n];
        if let Some(prng) = self.prngs
            .get_mut(&task_id) {
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
    TaskId: Ord + Copy + Clone + Default
{
    fn take_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }

    fn discard_current_and_restore_previous_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }

    fn restore_snapshot(&mut self) -> Result<(), crate::emulator::abstractions::errors::GenericError> {
        todo!()
    }
}

#[derive(Default)]
pub struct Seed<R>
where
    R: SeedableRng
{
    seed: <R as SeedableRng>::Seed
}

impl<R> fmt::Debug for Seed<R>
where
    R: SeedableRng
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
